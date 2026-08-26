//! StellarRail escrow contract.
//!
//! Module responsibilities:
//!
//! | Module | Owns |
//! |---|---|
//! | `lib` | `#[contract]` + `#[contractimpl]` entrypoints only (thin glue) |
//! | `types` | Canonical schema: `EscrowRequest`, `EscrowStatus`, `SCHEMA_VERSION` |
//! | `errors` | Typed `EscrowError` codes (stable discriminants) |
//! | `storage` | `DataKey`s, TTL-extending read/write helpers, counters |
//! | `events` | `#[contractevent]` structs, emitted on every mutation |
//! | `admin` | Roles: `initialize`, pause, rotation, upgrade auth |
//! | `validation` | Pure input checks: amounts, deadlines, pagination caps |
//! | `token` | SAC movement helpers (pull on deposit, push on settlement) |
//!
//! Rules: no `unwrap()`/`expect()` in non-test code, checked arithmetic,
//! TTL extended on every write, exactly one event per mutation.

#![no_std]

// Staged: role helpers are wired entrypoint-by-entrypoint across Phase B.
// Removed at ISSUE-028 (full clippy sweep proves every helper is used).
#[allow(dead_code)]
mod admin;
mod errors;
pub mod events;
pub mod storage;
// Staged: token helpers + fixtures are wired across Phase B (ISSUE-013+).
// Removed at ISSUE-028 via full clippy sweep.
#[allow(dead_code)]
mod token;
pub mod types;
mod validation;

#[cfg(test)]
#[allow(dead_code)] // staged: fixtures consumed across Phase B tests
pub(crate) mod fixtures;

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env};

pub use errors::EscrowError;
pub use types::{EscrowRequest, EscrowStatus, SCHEMA_VERSION};

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    /// One-time setup: roles + accepted SAC token. Second call fails
    /// `AlreadyExists`. `admin` must authorize.
    pub fn initialize(
        env: Env,
        admin: Address,
        signer: Address,
        token: Address,
    ) -> Result<(), EscrowError> {
        admin::do_initialize(&env, &admin, &signer, &token)
    }

    /// Lock `amount` stroops for `request_id`. Pulls the SAC token from
    /// `depositor` (who must authorize), persists a `Locked` request, and
    /// emits `FundsLocked`.
    pub fn deposit(
        env: Env,
        amount: i128,
        request_id: BytesN<32>,
        depositor: Address,
        destination: Option<Address>,
        deadline: u64,
    ) -> Result<EscrowRequest, EscrowError> {
        // Cheap gates first: pause, amount, id reuse, destination, deadline.
        if storage::is_paused(&env) {
            return Err(EscrowError::Paused);
        }
        validation::check_amount(amount)?;
        if storage::get_request(&env, &request_id).is_some() {
            return Err(EscrowError::AlreadyExists);
        }
        // v1 requires an explicit payout target (kept Option for API evolution).
        let destination = destination.ok_or(EscrowError::InvalidAmount)?;
        let now = env.ledger().timestamp();
        validation::check_deadline(deadline, now)?;
        depositor.require_auth();
        let token = storage::get_token(&env).ok_or(EscrowError::NotFound)?;

        // Effects before interactions (checks-effects-interactions).
        let request = EscrowRequest {
            request_id: request_id.clone(),
            depositor: depositor.clone(),
            destination: Some(destination),
            amount,
            deadline,
            status: EscrowStatus::Locked,
            created_at: now,
            updated_at: now,
        };
        storage::set_request(&env, &request);
        storage::track_request_id(&env, &request_id);
        let mut stats = storage::get_stats(&env);
        stats.locked_count = stats
            .locked_count
            .checked_add(1)
            .ok_or(EscrowError::Overflow)?;
        stats.locked_total = stats
            .locked_total
            .checked_add(amount)
            .ok_or(EscrowError::Overflow)?;
        stats.lifetime_volume = stats
            .lifetime_volume
            .checked_add(amount)
            .ok_or(EscrowError::Overflow)?;
        storage::set_stats(&env, &stats);

        // Interactions last: pull funds, then emit.
        token::pull(&env, &token, &depositor, amount);
        events::emit_funds_locked(&env, &request_id, amount, &depositor);
        Ok(request)
    }

    /// Settle a `Locked` request to its destination. Only the admin or the
    /// signer may call (`caller` must authorize). Past the deadline the
    /// request can only be expired — this returns `Expired` as a hint.
    pub fn release(
        env: Env,
        caller: Address,
        request_id: BytesN<32>,
    ) -> Result<EscrowRequest, EscrowError> {
        if storage::is_paused(&env) {
            return Err(EscrowError::Paused);
        }
        let mut request = storage::get_request(&env, &request_id).ok_or(EscrowError::NotFound)?;
        if request.status != EscrowStatus::Locked {
            return Err(EscrowError::InvalidState);
        }
        let now = env.ledger().timestamp();
        if now > request.deadline {
            return Err(EscrowError::Expired);
        }
        admin::require_admin_or_signer(&env, &caller)?;
        let token = storage::get_token(&env).ok_or(EscrowError::NotFound)?;
        // Unreachable by construction (deposit requires Some); kept as a
        // corruption guard instead of unwrap.
        let destination = request
            .destination
            .clone()
            .ok_or(EscrowError::InvalidState)?;

        // Effects before interactions.
        request.status = EscrowStatus::Released;
        request.updated_at = now;
        storage::set_request(&env, &request);
        let mut stats = storage::get_stats(&env);
        stats.locked_count = stats
            .locked_count
            .checked_sub(1)
            .ok_or(EscrowError::Overflow)?;
        stats.locked_total = stats
            .locked_total
            .checked_sub(request.amount)
            .ok_or(EscrowError::Overflow)?;
        stats.released_count = stats
            .released_count
            .checked_add(1)
            .ok_or(EscrowError::Overflow)?;
        storage::set_stats(&env, &stats);

        // Interactions last: pay out, then emit.
        token::push(&env, &token, &destination, request.amount);
        events::emit_payment_released(&env, &request_id, request.amount, &caller);
        Ok(request)
    }

    /// Return locked funds to the depositor (rejection path). Only the admin
    /// or the signer may call. Accepted from `Locked` at any time — including
    /// past the deadline; terminal states (`Released`/`Refunded`/`Expired`/
    /// `Failed`) fail `InvalidState`.
    ///
    /// Safety note: `expire` already transfers funds when it marks a request
    /// `Expired`, so `Expired` is terminal and NOT refundable — refunding from
    /// `Expired` would pay the depositor twice.
    pub fn refund(
        env: Env,
        caller: Address,
        request_id: BytesN<32>,
    ) -> Result<EscrowRequest, EscrowError> {
        if storage::is_paused(&env) {
            return Err(EscrowError::Paused);
        }
        let mut request = storage::get_request(&env, &request_id).ok_or(EscrowError::NotFound)?;
        if request.status != EscrowStatus::Locked {
            return Err(EscrowError::InvalidState);
        }
        admin::require_admin_or_signer(&env, &caller)?;
        let token = storage::get_token(&env).ok_or(EscrowError::NotFound)?;

        // Effects before interactions.
        let now = env.ledger().timestamp();
        request.status = EscrowStatus::Refunded;
        request.updated_at = now;
        storage::set_request(&env, &request);
        let mut stats = storage::get_stats(&env);
        stats.locked_count = stats
            .locked_count
            .checked_sub(1)
            .ok_or(EscrowError::Overflow)?;
        stats.locked_total = stats
            .locked_total
            .checked_sub(request.amount)
            .ok_or(EscrowError::Overflow)?;
        storage::set_stats(&env, &stats);

        // Interactions last: return funds, then emit.
        token::push(&env, &token, &request.depositor, request.amount);
        events::emit_payment_refunded(&env, &request_id, request.amount, &caller);
        Ok(request)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fixtures::{self, Actors};

    fn setup() -> (Env, Actors) {
        fixtures::setup_initialized()
    }

    #[test]
    fn deposit_validation_zero_amount() {
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        let res = c.try_deposit(
            &0,
            &fixtures::request_id(&env, 1),
            &actors.depositor,
            &Some(actors.destination.clone()),
            &(env.ledger().timestamp() + 3600),
        );
        assert_eq!(res, Err(Ok(EscrowError::InvalidAmount)));
    }

    #[test]
    fn deposit_validation_past_deadline() {
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        let now = env.ledger().timestamp();
        let res = c.try_deposit(
            &10_000_000,
            &fixtures::request_id(&env, 1),
            &actors.depositor,
            &Some(actors.destination.clone()),
            &now,
        );
        assert_eq!(res, Err(Ok(EscrowError::InvalidDeadline)));
    }

    #[test]
    fn deposit_validation_duplicate_id() {
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        fixtures::mint(&env, &actors, &actors.depositor, 100_000_000);
        let id = fixtures::request_id(&env, 1);
        let deadline = env.ledger().timestamp() + 3600;
        c.deposit(
            &10_000_000,
            &id,
            &actors.depositor,
            &Some(actors.destination.clone()),
            &deadline,
        );
        let res = c.try_deposit(
            &10_000_000,
            &id,
            &actors.depositor,
            &Some(actors.destination.clone()),
            &deadline,
        );
        assert_eq!(res, Err(Ok(EscrowError::AlreadyExists)));
    }

    #[test]
    fn deposit_validation_missing_destination() {
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        let res = c.try_deposit(
            &10_000_000,
            &fixtures::request_id(&env, 1),
            &actors.depositor,
            &None,
            &(env.ledger().timestamp() + 3600),
        );
        assert_eq!(res, Err(Ok(EscrowError::InvalidAmount)));
    }

    #[test]
    fn deposit_transfer_state_event() {
        use soroban_sdk::testutils::Events as _;
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        fixtures::mint(&env, &actors, &actors.depositor, 100_000_000);
        let before = fixtures::balance(&env, &actors, &actors.depositor);

        let amount = 10_000_000;
        let id = fixtures::request_id(&env, 7);
        let now = env.ledger().timestamp();
        let deadline = now + 3600;
        let req = c.deposit(
            &amount,
            &id,
            &actors.depositor,
            &Some(actors.destination.clone()),
            &deadline,
        );

        // Exactly one FundsLocked event. This must be read before ANY further
        // contract invocation (even a token balance read resets the window).
        let events = env.events().all().filter_by_contract(&actors.contract_id);
        assert_eq!(events.events().len(), 1);

        // Funds moved depositor -> contract.
        assert_eq!(
            fixtures::balance(&env, &actors, &actors.depositor),
            before - amount
        );
        assert_eq!(
            fixtures::balance(&env, &actors, &actors.contract_id),
            amount
        );

        // Persisted request is Locked with exact fields.
        assert_eq!(req.status, EscrowStatus::Locked);
        assert_eq!(req.amount, amount);
        assert_eq!(req.deadline, deadline);
        assert_eq!(req.created_at, now);
        assert_eq!(req.updated_at, now);

        let stored = env.as_contract(&actors.contract_id, || {
            crate::storage::get_request(&env, &id)
        });
        assert_eq!(stored, Some(req));

        // Counters updated.
        let stats = env.as_contract(&actors.contract_id, || crate::storage::get_stats(&env));
        assert_eq!(stats.locked_count, 1);
        assert_eq!(stats.locked_total, amount);
        assert_eq!(stats.lifetime_volume, amount);
    }

    fn fund_and_deposit(env: &Env, actors: &Actors, byte: u8, amount: i128) -> BytesN<32> {
        fixtures::mint(env, actors, &actors.depositor, amount * 10);
        let c = fixtures::client(env, actors);
        let id = fixtures::request_id(env, byte);
        c.deposit(
            &amount,
            &id,
            &actors.depositor,
            &Some(actors.destination.clone()),
            &(env.ledger().timestamp() + 3600),
        );
        id
    }

    #[test]
    fn release_guards_non_admin() {
        use soroban_sdk::testutils::Address as _;
        let (env, actors) = setup();
        let id = fund_and_deposit(&env, &actors, 11, 10_000_000);

        // Auth passes for a random caller, but the role check must fail typed.
        fixtures::clear_auth_mock(&env);
        let rogue = Address::generate(&env);
        fixtures::mock_release(&env, &actors, &rogue, &id);
        let c = fixtures::client(&env, &actors);
        let res = c.try_release(&rogue, &id);
        assert_eq!(res, Err(Ok(EscrowError::Unauthorized)));
    }

    #[test]
    fn release_guards_double_release() {
        let (env, actors) = setup();
        let id = fund_and_deposit(&env, &actors, 12, 10_000_000);
        let c = fixtures::client(&env, &actors);
        c.release(&actors.admin, &id);
        let res = c.try_release(&actors.admin, &id);
        assert_eq!(res, Err(Ok(EscrowError::InvalidState)));
    }

    #[test]
    fn release_guards_expired_deadline() {
        use soroban_sdk::testutils::Ledger as _;
        let (env, actors) = setup();
        let id = fund_and_deposit(&env, &actors, 13, 10_000_000);
        let c = fixtures::client(&env, &actors);
        env.ledger().set_timestamp(env.ledger().timestamp() + 3601);
        let res = c.try_release(&actors.admin, &id);
        assert_eq!(res, Err(Ok(EscrowError::Expired)));
    }

    #[test]
    fn release_guards_unknown_request() {
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        let res = c.try_release(&actors.admin, &fixtures::request_id(&env, 99));
        assert_eq!(res, Err(Ok(EscrowError::NotFound)));
    }

    #[test]
    fn release_transfer_event_status() {
        use soroban_sdk::testutils::Events as _;
        let (env, actors) = setup();
        let amount = 25_000_000;
        let id = fund_and_deposit(&env, &actors, 14, amount);
        let c = fixtures::client(&env, &actors);
        let dest_before = fixtures::balance(&env, &actors, &actors.destination);

        let req = c.release(&actors.admin, &id);

        // Exactly one PaymentReleased event (read before any further
        // invocation, which would reset the event window).
        let events = env.events().all().filter_by_contract(&actors.contract_id);
        assert_eq!(events.events().len(), 1);

        // Destination paid exactly once.
        assert_eq!(
            fixtures::balance(&env, &actors, &actors.destination),
            dest_before + amount
        );
        // Contract holds nothing for this request anymore.
        assert_eq!(fixtures::balance(&env, &actors, &actors.contract_id), 0);

        // Terminal state persisted.
        assert_eq!(req.status, EscrowStatus::Released);
        assert_eq!(req.updated_at, env.ledger().timestamp());
        let stats = env.as_contract(&actors.contract_id, || crate::storage::get_stats(&env));
        assert_eq!(stats.locked_count, 0);
        assert_eq!(stats.locked_total, 0);
        assert_eq!(stats.released_count, 1);
        assert_eq!(stats.lifetime_volume, amount);
    }

    #[test]
    fn release_by_signer_settles() {
        let (env, actors) = setup();
        let amount = 5_000_000;
        let id = fund_and_deposit(&env, &actors, 15, amount);
        let c = fixtures::client(&env, &actors);
        let dest_before = fixtures::balance(&env, &actors, &actors.destination);
        let req = c.release(&actors.signer, &id);
        assert_eq!(req.status, EscrowStatus::Released);
        assert_eq!(
            fixtures::balance(&env, &actors, &actors.destination),
            dest_before + amount
        );
    }

    #[test]
    fn refund_guards_non_admin() {
        use soroban_sdk::testutils::Address as _;
        let (env, actors) = setup();
        let id = fund_and_deposit(&env, &actors, 16, 10_000_000);

        fixtures::clear_auth_mock(&env);
        let rogue = Address::generate(&env);
        fixtures::mock_refund(&env, &actors, &rogue, &id);
        let c = fixtures::client(&env, &actors);
        let res = c.try_refund(&rogue, &id);
        assert_eq!(res, Err(Ok(EscrowError::Unauthorized)));
    }

    #[test]
    fn refund_guards_after_release() {
        let (env, actors) = setup();
        let id = fund_and_deposit(&env, &actors, 17, 10_000_000);
        let c = fixtures::client(&env, &actors);
        c.release(&actors.admin, &id);
        let res = c.try_refund(&actors.admin, &id);
        assert_eq!(res, Err(Ok(EscrowError::InvalidState)));
    }

    #[test]
    fn refund_guards_double_refund() {
        let (env, actors) = setup();
        let id = fund_and_deposit(&env, &actors, 18, 10_000_000);
        let c = fixtures::client(&env, &actors);
        c.refund(&actors.signer, &id);
        let res = c.try_refund(&actors.signer, &id);
        assert_eq!(res, Err(Ok(EscrowError::InvalidState)));
    }

    #[test]
    fn refund_guards_unknown_request() {
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        let res = c.try_refund(&actors.admin, &fixtures::request_id(&env, 99));
        assert_eq!(res, Err(Ok(EscrowError::NotFound)));
    }

    #[test]
    fn refund_transfer_event_status() {
        use soroban_sdk::testutils::Events as _;
        let (env, actors) = setup();
        let amount = 30_000_000;
        let id = fund_and_deposit(&env, &actors, 19, amount);
        let c = fixtures::client(&env, &actors);
        let depositor_before = fixtures::balance(&env, &actors, &actors.depositor);

        let req = c.refund(&actors.signer, &id);

        // Exactly one PaymentRefunded event (read before balance calls).
        let events = env.events().all().filter_by_contract(&actors.contract_id);
        assert_eq!(events.events().len(), 1);

        // Depositor made whole; contract holds nothing.
        assert_eq!(
            fixtures::balance(&env, &actors, &actors.depositor),
            depositor_before + amount
        );
        assert_eq!(fixtures::balance(&env, &actors, &actors.contract_id), 0);

        // Terminal state persisted; release counter untouched.
        assert_eq!(req.status, EscrowStatus::Refunded);
        let stats = env.as_contract(&actors.contract_id, || crate::storage::get_stats(&env));
        assert_eq!(stats.locked_count, 0);
        assert_eq!(stats.locked_total, 0);
        assert_eq!(stats.released_count, 0);
        assert_eq!(stats.lifetime_volume, amount);
    }

    #[test]
    fn refund_after_deadline_still_settles() {
        use soroban_sdk::testutils::Ledger as _;
        // Rejection path stays open past the deadline (status is still Locked).
        let (env, actors) = setup();
        let amount = 8_000_000;
        let id = fund_and_deposit(&env, &actors, 20, amount);
        env.ledger().set_timestamp(env.ledger().timestamp() + 7200);
        let c = fixtures::client(&env, &actors);
        let req = c.refund(&actors.admin, &id);
        assert_eq!(req.status, EscrowStatus::Refunded);
    }
}
