//! `StellarRail` escrow contract.
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
// Contract signatures must take `Env`, `Address`, `BytesN`, and `Option`
// by value (Soroban host dispatch); `needless_pass_by_value` therefore
// cannot apply to entrypoints, and helpers mirror entrypoint signatures
// for consistency.
#![allow(clippy::needless_pass_by_value)]

// All role helpers are now wired to entrypoints (Phase B complete).
mod admin;
mod errors;
pub mod events;
pub mod storage;
mod token;
pub mod types;
/// Pure input checks (amounts, deadlines, caps). Public so integrators can
/// pre-validate; also the single source of truth for bound constants.
pub mod validation;

#[cfg(test)]
pub(crate) mod fixtures;

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Vec};

pub use errors::EscrowError;
pub use types::{EscrowRequest, EscrowStatus, SCHEMA_VERSION};

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    /// One-time setup: roles + accepted SAC token. Second call fails
    /// `AlreadyExists`. `admin` must authorize.
    ///
    /// # Errors
    ///
    /// Returns [`EscrowError::AlreadyExists`] on second call.
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
    ///
    /// # Errors
    ///
    /// Returns `Paused`, `InvalidAmount`, `AlreadyExists`, `InvalidDeadline`,
    /// `NotFound` (uninitialized), or `Overflow` per `docs/ERRORS.md`.
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
    ///
    /// # Errors
    ///
    /// Returns `Paused`, `NotFound`, `InvalidState`, `Expired`,
    /// `Unauthorized`, or `Overflow` per `docs/ERRORS.md`.
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
    ///
    /// # Errors
    ///
    /// Returns `Paused`, `NotFound`, `InvalidState`, `Unauthorized`, or
    /// `Overflow` per `docs/ERRORS.md`.
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

    /// Rescue past-due funds. Callable by ANYONE (no auth) — it only ever
    /// pays the original depositor, so there is nothing to steal. Requires
    /// `now > deadline` and `Locked` status; works while paused so funds
    /// always remain rescuable.
    ///
    /// # Errors
    ///
    /// Returns `NotFound`, `InvalidState`, `NotExpired`, or `Overflow` per
    /// `docs/ERRORS.md`.
    pub fn expire(env: Env, request_id: BytesN<32>) -> Result<EscrowRequest, EscrowError> {
        let mut request = storage::get_request(&env, &request_id).ok_or(EscrowError::NotFound)?;
        if request.status != EscrowStatus::Locked {
            return Err(EscrowError::InvalidState);
        }
        let now = env.ledger().timestamp();
        if now <= request.deadline {
            return Err(EscrowError::NotExpired);
        }
        let token = storage::get_token(&env).ok_or(EscrowError::NotFound)?;

        // Effects before interactions.
        request.status = EscrowStatus::Expired;
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

        // Interactions last: return funds, then emit. The event actor is the
        // refunded depositor — expire itself is permissionless.
        token::push(&env, &token, &request.depositor, request.amount);
        events::emit_request_expired(&env, &request_id, request.amount, &request.depositor);
        Ok(request)
    }

    /// Read a request. No auth, no writes, no TTL bump (strictly read-only).
    ///
    /// # Errors
    ///
    /// Returns [`EscrowError::NotFound`] for an unknown `request_id`.
    pub fn get_request(env: Env, request_id: BytesN<32>) -> Result<EscrowRequest, EscrowError> {
        storage::get_request(&env, &request_id).ok_or(EscrowError::NotFound)
    }

    /// Paginate the request registry in insertion order. `offset`/`limit`
    /// slice the raw registry (bounded work: at most `limit` reads); the
    /// status filter drops non-matching entries within the window, so
    /// filtered scans walk pages until a short page. `limit` is capped at
    /// `MAX_LIST_LIMIT` (50); `limit == 0` or `offset` past the end yields
    /// an empty page. Read-only.
    ///
    /// # Errors
    ///
    /// Returns `InvalidAmount` when `limit` exceeds the cap, or `Overflow`
    /// when `offset + limit` overflows `u32`.
    pub fn list_requests(
        env: Env,
        status_filter: Option<EscrowStatus>,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<EscrowRequest>, EscrowError> {
        use crate::validation::MAX_LIST_LIMIT;
        if limit > MAX_LIST_LIMIT {
            return Err(EscrowError::InvalidAmount);
        }
        let ids = storage::get_all_ids(&env);
        let total = ids.len();
        let mut out = Vec::new(&env);
        if limit == 0 || offset >= total {
            return Ok(out);
        }
        let window = offset.checked_add(limit).ok_or(EscrowError::Overflow)?;
        let end = if window < total { window } else { total };
        let mut i = offset;
        while i < end {
            let id = ids.get(i).ok_or(EscrowError::NotFound)?;
            if let Some(req) = storage::get_request(&env, &id) {
                let keep = match &status_filter {
                    None => true,
                    Some(want) => *want == req.status,
                };
                if keep {
                    out.push_back(req);
                }
            }
            i += 1;
        }
        Ok(out)
    }

    /// Flip the emergency stop (admin only). While paused, `deposit`,
    /// `release`, and `refund` fail `Paused`; `expire` keeps working so
    /// funds always remain rescuable. Emits `Paused` / `Unpaused`.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` (uninitialized) or `Unauthorized` (not admin).
    pub fn set_paused(env: Env, caller: Address, paused: bool) -> Result<(), EscrowError> {
        admin::do_set_paused(&env, &caller, paused)
    }

    /// Stage a new admin (current admin only). Takes effect on `accept_admin`.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` (uninitialized) or `Unauthorized` (not admin).
    pub fn transfer_admin(
        env: Env,
        caller: Address,
        new_admin: Address,
    ) -> Result<(), EscrowError> {
        admin::do_transfer_admin(&env, &caller, &new_admin)
    }

    /// Complete admin rotation: the staged admin authorizes acceptance.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` (no rotation staged) or `Unauthorized` (caller is
    /// not the staged admin).
    pub fn accept_admin(env: Env, caller: Address) -> Result<(), EscrowError> {
        admin::do_accept_admin(&env, &caller)
    }

    /// Rotate the signer (admin only). The old signer is invalid immediately.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` (uninitialized) or `Unauthorized` (not admin).
    pub fn set_signer(env: Env, caller: Address, new_signer: Address) -> Result<(), EscrowError> {
        admin::do_set_signer(&env, &caller, &new_signer)
    }

    /// Storage schema version. The release workflow refuses to cut a release
    /// whose tag disagrees with this value.
    #[must_use]
    pub fn version(env: Env) -> u32 {
        let _ = env;
        SCHEMA_VERSION
    }

    /// Upgrade the contract WASM (admin only). State is preserved: all data
    /// lives in instance-independent persistent storage keyed by `DataKey`,
    /// so a new `SCHEMA_VERSION` must keep every key readable (see
    /// `docs/UPGRADE.md`).
    ///
    /// # Errors
    ///
    /// Returns `NotFound` (uninitialized) or `Unauthorized` (not admin).
    pub fn migrate(
        env: Env,
        caller: Address,
        new_wasm_hash: BytesN<32>,
    ) -> Result<(), EscrowError> {
        admin::do_migrate(&env, &caller, &new_wasm_hash)
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

    #[test]
    fn expire_rejects_early() {
        let (env, actors) = setup();
        let id = fund_and_deposit(&env, &actors, 21, 10_000_000);
        let c = fixtures::client(&env, &actors);
        let res = c.try_expire(&id);
        assert_eq!(res, Err(Ok(EscrowError::NotExpired)));
    }

    #[test]
    fn expire_by_third_party_refunds_depositor() {
        use soroban_sdk::testutils::{Events as _, Ledger as _};
        let (env, actors) = setup();
        let amount = 12_000_000;
        let id = fund_and_deposit(&env, &actors, 22, amount);
        env.ledger().set_timestamp(env.ledger().timestamp() + 7200);

        // No auth is configured at all here: expire is permissionless.
        // (clear_auth_mock dropped mock_all_auths; no mock_auths installed.)
        fixtures::clear_auth_mock(&env);
        let c = fixtures::client(&env, &actors);
        let depositor_before = fixtures::balance(&env, &actors, &actors.depositor);
        let req = c.expire(&id);

        // Exactly one RequestExpired event (read before balance calls).
        let events = env.events().all().filter_by_contract(&actors.contract_id);
        assert_eq!(events.events().len(), 1);

        assert_eq!(req.status, EscrowStatus::Expired);
        assert_eq!(
            fixtures::balance(&env, &actors, &actors.depositor),
            depositor_before + amount
        );
        assert_eq!(fixtures::balance(&env, &actors, &actors.contract_id), 0);
    }

    #[test]
    fn expire_twice_fails() {
        use soroban_sdk::testutils::Ledger as _;
        let (env, actors) = setup();
        let id = fund_and_deposit(&env, &actors, 23, 10_000_000);
        env.ledger().set_timestamp(env.ledger().timestamp() + 7200);
        let c = fixtures::client(&env, &actors);
        c.expire(&id);
        let res = c.try_expire(&id);
        assert_eq!(res, Err(Ok(EscrowError::InvalidState)));
    }

    #[test]
    fn expire_after_release_fails() {
        use soroban_sdk::testutils::Ledger as _;
        let (env, actors) = setup();
        let id = fund_and_deposit(&env, &actors, 24, 10_000_000);
        let c = fixtures::client(&env, &actors);
        c.release(&actors.admin, &id);
        // Push past the deadline: released funds must NOT move again.
        env.ledger().set_timestamp(env.ledger().timestamp() + 7200);
        let res = c.try_expire(&id);
        assert_eq!(res, Err(Ok(EscrowError::InvalidState)));
    }

    #[test]
    fn get_request_returns_stored() {
        let (env, actors) = setup();
        let id = fund_and_deposit(&env, &actors, 25, 10_000_000);
        let c = fixtures::client(&env, &actors);
        let req = c.get_request(&id);
        assert_eq!(req.request_id, id);
        assert_eq!(req.depositor, actors.depositor);
        assert_eq!(req.destination, Some(actors.destination.clone()));
        assert_eq!(req.status, EscrowStatus::Locked);
    }

    #[test]
    fn get_request_unknown_fails() {
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        let res = c.try_get_request(&fixtures::request_id(&env, 99));
        assert_eq!(res, Err(Ok(EscrowError::NotFound)));
    }

    #[test]
    fn list_requests_pages_seeded_registry() {
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        fixtures::mint(&env, &actors, &actors.depositor, 1_000_000_000);
        // 25 deposits: bytes 100..125.
        for byte in 100u8..125u8 {
            c.deposit(
                &1_000_000,
                &fixtures::request_id(&env, byte),
                &actors.depositor,
                &Some(actors.destination.clone()),
                &(env.ledger().timestamp() + 3600),
            );
        }
        // Settle: first 10 released, next 5 refunded, last 10 stay locked.
        for byte in 100u8..110u8 {
            c.release(&actors.admin, &fixtures::request_id(&env, byte));
        }
        for byte in 110u8..115u8 {
            c.refund(&actors.signer, &fixtures::request_id(&env, byte));
        }

        let page1 = c.list_requests(&None, &0, &10);
        assert_eq!(page1.len(), 10);
        assert_eq!(
            page1.get(0).unwrap().request_id,
            fixtures::request_id(&env, 100)
        );
        let page2 = c.list_requests(&None, &10, &10);
        assert_eq!(page2.len(), 10);
        assert_eq!(
            page2.get(0).unwrap().request_id,
            fixtures::request_id(&env, 110)
        );
        let page3 = c.list_requests(&None, &20, &10);
        assert_eq!(page3.len(), 5);

        // Filtered: all 10 released across one window.
        let released = c.list_requests(&Some(EscrowStatus::Released), &0, &50);
        assert_eq!(released.len(), 10);
        // Filtered window over locked tail (ids 115..125 are all Locked).
        let locked_tail = c.list_requests(&Some(EscrowStatus::Locked), &15, &10);
        assert_eq!(locked_tail.len(), 10);
        // Filtered window with no matches.
        let empty = c.list_requests(&Some(EscrowStatus::Locked), &0, &10);
        assert_eq!(empty.len(), 0);

        // Offset past the end and zero limit yield empty pages.
        assert_eq!(c.list_requests(&None, &100, &10).len(), 0);
        assert_eq!(c.list_requests(&None, &0, &0).len(), 0);
    }

    #[test]
    fn list_requests_rejects_over_cap() {
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        let res = c.try_list_requests(&None, &0, &1000);
        assert_eq!(res, Err(Ok(EscrowError::InvalidAmount)));
    }

    #[test]
    fn deposit_deadline_bounds_enforced() {
        let (env, actors) = setup();
        fixtures::mint(&env, &actors, &actors.depositor, 1_000_000_000);
        let c = fixtures::client(&env, &actors);
        let now = env.ledger().timestamp();
        let dest = Some(actors.destination.clone());

        // Under the 60s minimum.
        let short = c.try_deposit(
            &1_000_000,
            &fixtures::request_id(&env, 30),
            &actors.depositor,
            &dest,
            &(now + 30),
        );
        assert_eq!(short, Err(Ok(EscrowError::InvalidDeadline)));

        // Over the 30-day maximum (31 days, and a 10-year deadline).
        for (byte, delta) in [(31u8, 31 * 24 * 60 * 60), (32u8, 10 * 365 * 24 * 60 * 60)] {
            let res = c.try_deposit(
                &1_000_000,
                &fixtures::request_id(&env, byte),
                &actors.depositor,
                &dest,
                &(now + delta),
            );
            assert_eq!(res, Err(Ok(EscrowError::InvalidDeadline)));
        }

        // Exact minimum lifetime is accepted.
        c.deposit(
            &1_000_000,
            &fixtures::request_id(&env, 33),
            &actors.depositor,
            &dest,
            &(now + 60),
        );
    }

    #[test]
    fn amounts_overflow_guarded() {
        let (env, actors) = setup();
        fixtures::mint(&env, &actors, &actors.depositor, 1_000_000_000);
        // Push a counter to the edge: the next deposit must fail Overflow
        // (and roll back the request write atomically).
        env.as_contract(&actors.contract_id, || {
            let mut stats = crate::storage::get_stats(&env);
            stats.lifetime_volume = i128::MAX;
            crate::storage::set_stats(&env, &stats);
        });
        let c = fixtures::client(&env, &actors);
        let id = fixtures::request_id(&env, 34);
        let res = c.try_deposit(
            &1_000_000,
            &id,
            &actors.depositor,
            &Some(actors.destination.clone()),
            &(env.ledger().timestamp() + 3600),
        );
        assert_eq!(res, Err(Ok(EscrowError::Overflow)));
        // Rolled back: no request persisted.
        let stored = env.as_contract(&actors.contract_id, || {
            crate::storage::get_request(&env, &id)
        });
        assert_eq!(stored, None);
    }

    #[test]
    fn reentrancy_failed_transfer_rolls_back() {
        use soroban_sdk::testutils::Events as _;
        // Checks-effects-interactions + host atomicity: the request write and
        // the token pull are one transaction. Here the pull must fail
        // (depositor was never funded), so nothing may persist — no request,
        // no counters, no events. A reentrant/callback-style partial state is
        // impossible: the host forbids reentering the contract mid-call and
        // rolls back the whole invocation on any failure.
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        let id = fixtures::request_id(&env, 35);
        let res = c.try_deposit(
            &1_000_000,
            &id,
            &actors.depositor,
            &Some(actors.destination.clone()),
            &(env.ledger().timestamp() + 3600),
        );
        assert!(res.is_err(), "underfunded pull must fail");
        let stored = env.as_contract(&actors.contract_id, || {
            crate::storage::get_request(&env, &id)
        });
        assert_eq!(stored, None);
        let stats = env.as_contract(&actors.contract_id, || crate::storage::get_stats(&env));
        assert_eq!(stats.lifetime_volume, 0);
        assert_eq!(env.events().all().events().len(), 0);
    }

    #[test]
    fn pause_blocks_mutations_but_not_expire() {
        use soroban_sdk::testutils::{Events as _, Ledger as _};
        let (env, actors) = setup();
        let id = fund_and_deposit(&env, &actors, 36, 10_000_000);
        let c = fixtures::client(&env, &actors);

        c.set_paused(&actors.admin, &true);
        // One Paused event.
        assert_eq!(
            env.events()
                .all()
                .filter_by_contract(&actors.contract_id)
                .events()
                .len(),
            1
        );

        // Mutations blocked with typed Paused.
        let deposit_res = c.try_deposit(
            &1_000_000,
            &fixtures::request_id(&env, 37),
            &actors.depositor,
            &Some(actors.destination.clone()),
            &(env.ledger().timestamp() + 3600),
        );
        assert_eq!(deposit_res, Err(Ok(EscrowError::Paused)));
        assert_eq!(
            c.try_release(&actors.admin, &id),
            Err(Ok(EscrowError::Paused))
        );
        assert_eq!(
            c.try_refund(&actors.admin, &id),
            Err(Ok(EscrowError::Paused))
        );

        // Expire still rescues past-due funds while paused.
        env.ledger().set_timestamp(env.ledger().timestamp() + 7200);
        let req = c.expire(&id);
        assert_eq!(req.status, EscrowStatus::Expired);
    }

    #[test]
    fn unpause_resumes_and_emits() {
        use soroban_sdk::testutils::Events as _;
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        c.set_paused(&actors.admin, &true);
        c.set_paused(&actors.admin, &false);
        // The last invocation (unpause) emitted exactly one Unpaused event.
        assert_eq!(
            env.events()
                .all()
                .filter_by_contract(&actors.contract_id)
                .events()
                .len(),
            1
        );
        // Mutations work again.
        fund_and_deposit(&env, &actors, 38, 10_000_000);
    }

    #[test]
    fn signer_rotation_invalidates_old_signer() {
        use soroban_sdk::testutils::{Address as _, Events as _};
        let (env, actors) = setup();
        let id = fund_and_deposit(&env, &actors, 39, 10_000_000);
        let c = fixtures::client(&env, &actors);

        let new_signer = Address::generate(&env);
        c.set_signer(&actors.admin, &new_signer);
        // One SignerUpdated event on the last call.
        assert_eq!(
            env.events()
                .all()
                .filter_by_contract(&actors.contract_id)
                .events()
                .len(),
            1
        );

        // New signer settles a fresh request...
        let id2 = fund_and_deposit(&env, &actors, 40, 5_000_000);
        let req = c.release(&new_signer, &id2);
        assert_eq!(req.status, EscrowStatus::Released);

        // ...but the old signer is immediately unauthorized (typed).
        fixtures::clear_auth_mock(&env);
        fixtures::mock_release(&env, &actors, &actors.signer, &id);
        let res = c.try_release(&actors.signer, &id);
        assert_eq!(res, Err(Ok(EscrowError::Unauthorized)));
    }

    #[test]
    fn admin_rotation_two_step() {
        use soroban_sdk::testutils::{Address as _, Events as _};
        use soroban_sdk::{IntoVal, Val, Vec as SdkVec};
        let (env, actors) = fixtures::setup_initialized();
        fixtures::clear_auth_mock(&env);
        let c = fixtures::client(&env, &actors);
        let new_admin = Address::generate(&env);
        let rogue = Address::generate(&env);

        // Every privileged call below installs its own exact mock.
        let mock = |fn_name: &str, args: SdkVec<Val>, caller: &Address| {
            fixtures::mock_single_call(&env, &actors.contract_id, fn_name, args, caller);
        };

        // Stage rotation as the current admin.
        mock(
            "transfer_admin",
            (actors.admin.clone(), new_admin.clone()).into_val(&env),
            &actors.admin,
        );
        c.transfer_admin(&actors.admin, &new_admin);

        // Staging hands over nothing yet: old admin still pauses.
        mock(
            "set_paused",
            (actors.admin.clone(), true).into_val(&env),
            &actors.admin,
        );
        c.set_paused(&actors.admin, &true);

        // Acceptance by anyone else fails typed.
        mock("accept_admin", (rogue.clone(),).into_val(&env), &rogue);
        assert_eq!(
            c.try_accept_admin(&rogue),
            Err(Ok(EscrowError::Unauthorized))
        );

        // Staged admin accepts.
        mock(
            "accept_admin",
            (new_admin.clone(),).into_val(&env),
            &new_admin,
        );
        c.accept_admin(&new_admin);

        // Old admin is now unauthorized...
        mock(
            "set_paused",
            (actors.admin.clone(), false).into_val(&env),
            &actors.admin,
        );
        assert_eq!(
            c.try_set_paused(&actors.admin, &false),
            Err(Ok(EscrowError::Unauthorized))
        );

        // ...and the new admin governs (unpauses).
        mock(
            "set_paused",
            (new_admin.clone(), false).into_val(&env),
            &new_admin,
        );
        c.set_paused(&new_admin, &false);
        let events = env.events().all().filter_by_contract(&actors.contract_id);
        assert_eq!(events.events().len(), 1);
    }

    #[test]
    fn version_returns_schema_version() {
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        assert_eq!(c.version(), SCHEMA_VERSION);
        assert_eq!(c.version(), 1);
    }

    #[test]
    fn migrate_requires_admin() {
        use soroban_sdk::testutils::Address as _;
        use soroban_sdk::IntoVal;
        let (env, actors) = setup();
        fixtures::clear_auth_mock(&env);
        let rogue = Address::generate(&env);
        let hash = BytesN::from_array(&env, &[0xab; 32]);
        fixtures::mock_single_call(
            &env,
            &actors.contract_id,
            "migrate",
            (rogue.clone(), hash.clone()).into_val(&env),
            &rogue,
        );
        let c = fixtures::client(&env, &actors);
        assert_eq!(
            c.try_migrate(&rogue, &hash),
            Err(Ok(EscrowError::Unauthorized))
        );
    }

    #[test]
    fn migrate_by_admin_passes_auth_gate() {
        use soroban_sdk::IntoVal;
        // No code was ever uploaded for this hash, so the host upgrade
        // itself fails — but it must fail PAST the auth gate (anything but
        // Unauthorized), proving admins can reach `migrate` while rogues
        // cannot (see `migrate_requires_admin`). A full state-preserving
        // upgrade is drilled in ISSUE-050 against real built WASM.
        let (env, actors) = setup();
        let hash = BytesN::from_array(&env, &[0xab; 32]);
        fixtures::clear_auth_mock(&env);
        fixtures::mock_single_call(
            &env,
            &actors.contract_id,
            "migrate",
            (actors.admin.clone(), hash.clone()).into_val(&env),
            &actors.admin,
        );
        let c = fixtures::client(&env, &actors);
        let res = c.try_migrate(&actors.admin, &hash);
        assert!(res.is_err());
        assert_ne!(res, Err(Ok(EscrowError::Unauthorized)));
    }

    /// Full access-control matrix: admin / signer / stranger ×
    /// `release` / `refund` / `expire` / `set_paused` (12 combos).
    ///
    /// Mirrors `docs/AUTH_MATRIX.md`. Stranger calls install an exact mock
    /// (auth passes) so the *role* check — not the host auth trap — is what
    /// denies them with typed `Unauthorized`.
    #[test]
    fn auth_matrix() {
        use soroban_sdk::testutils::{Address as _, Ledger as _};
        use soroban_sdk::IntoVal;
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        let rogue = Address::generate(&env);

        // Nine funded requests, all Locked.
        for byte in 50u8..59u8 {
            fund_and_deposit(&env, &actors, byte, 10_000_000);
        }
        let id = |b: u8| fixtures::request_id(&env, b);

        // release: admin + signer settle; stranger is denied typed.
        assert_eq!(
            c.release(&actors.admin, &id(50)).status,
            EscrowStatus::Released
        );
        assert_eq!(
            c.release(&actors.signer, &id(51)).status,
            EscrowStatus::Released
        );
        fixtures::clear_auth_mock(&env);
        fixtures::mock_release(&env, &actors, &rogue, &id(52));
        assert_eq!(
            c.try_release(&rogue, &id(52)),
            Err(Ok(EscrowError::Unauthorized))
        );

        // refund: same shape.
        env.mock_all_auths();
        assert_eq!(
            c.refund(&actors.admin, &id(53)).status,
            EscrowStatus::Refunded
        );
        assert_eq!(
            c.refund(&actors.signer, &id(54)).status,
            EscrowStatus::Refunded
        );
        fixtures::clear_auth_mock(&env);
        fixtures::mock_refund(&env, &actors, &rogue, &id(55));
        assert_eq!(
            c.try_refund(&rogue, &id(55)),
            Err(Ok(EscrowError::Unauthorized))
        );

        // set_paused: admin only.
        env.mock_all_auths();
        c.set_paused(&actors.admin, &true);
        fixtures::clear_auth_mock(&env);
        fixtures::mock_single_call(
            &env,
            &actors.contract_id,
            "set_paused",
            (actors.signer.clone(), true).into_val(&env),
            &actors.signer,
        );
        assert_eq!(
            c.try_set_paused(&actors.signer, &true),
            Err(Ok(EscrowError::Unauthorized))
        );
        fixtures::mock_single_call(
            &env,
            &actors.contract_id,
            "set_paused",
            (rogue.clone(), true).into_val(&env),
            &rogue,
        );
        assert_eq!(
            c.try_set_paused(&rogue, &true),
            Err(Ok(EscrowError::Unauthorized))
        );

        // expire: permissionless — succeeds under blanket mock, under an
        // unrelated single mock, and with no mocks at all.
        env.mock_all_auths();
        env.ledger().set_timestamp(env.ledger().timestamp() + 7200);
        assert_eq!(c.expire(&id(56)).status, EscrowStatus::Expired);
        fixtures::clear_auth_mock(&env);
        fixtures::mock_release(&env, &actors, &rogue, &id(57));
        assert_eq!(c.expire(&id(57)).status, EscrowStatus::Expired);
        fixtures::clear_auth_mock(&env);
        assert_eq!(c.expire(&id(58)).status, EscrowStatus::Expired);
    }

    /// Double-spend impossibility: every second attempt fails AND moves no
    /// funds (balances bit-identical before/after the rejected call).
    #[test]
    fn edge_double_ops_move_no_funds() {
        use soroban_sdk::testutils::Ledger as _;
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        let bal = |who: &Address| fixtures::balance(&env, &actors, who);

        // Double deposit: pulled exactly once.
        let id_a = fund_and_deposit(&env, &actors, 60, 10_000_000);
        let dep_after_first = bal(&actors.depositor);
        let res = c.try_deposit(
            &10_000_000,
            &id_a,
            &actors.depositor,
            &Some(actors.destination.clone()),
            &(env.ledger().timestamp() + 3600),
        );
        assert_eq!(res, Err(Ok(EscrowError::AlreadyExists)));
        assert_eq!(bal(&actors.depositor), dep_after_first);

        // Double release: destination paid exactly once.
        c.release(&actors.admin, &id_a);
        let dest_paid = bal(&actors.destination);
        assert_eq!(
            c.try_release(&actors.admin, &id_a),
            Err(Ok(EscrowError::InvalidState))
        );
        assert_eq!(bal(&actors.destination), dest_paid);
        assert_eq!(bal(&actors.contract_id), 0);

        // Double refund: depositor refunded exactly once.
        let id_b = fund_and_deposit(&env, &actors, 61, 7_000_000);
        c.refund(&actors.signer, &id_b);
        let dep_refunded = bal(&actors.depositor);
        assert_eq!(
            c.try_refund(&actors.signer, &id_b),
            Err(Ok(EscrowError::InvalidState))
        );
        assert_eq!(bal(&actors.depositor), dep_refunded);

        // Double expire: depositor rescued exactly once.
        let id_c = fund_and_deposit(&env, &actors, 62, 5_000_000);
        env.ledger().set_timestamp(env.ledger().timestamp() + 7200);
        c.expire(&id_c);
        let dep_expired = bal(&actors.depositor);
        assert_eq!(c.try_expire(&id_c), Err(Ok(EscrowError::InvalidState)));
        assert_eq!(bal(&actors.depositor), dep_expired);
        assert_eq!(bal(&actors.contract_id), 0);
    }

    #[test]
    fn overflow_deposit_i128_max_rejected() {
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        let res = c.try_deposit(
            &i128::MAX,
            &fixtures::request_id(&env, 63),
            &actors.depositor,
            &Some(actors.destination.clone()),
            &(env.ledger().timestamp() + 3600),
        );
        assert_eq!(res, Err(Ok(EscrowError::InvalidAmount)));
    }

    #[test]
    fn overflow_release_underflow_guarded() {
        // Corrupted counter (locked_total = 0 with a live Locked request):
        // settlement math must fail Overflow, never wrap — and move no funds.
        let (env, actors) = setup();
        let amount = 10_000_000;
        let id = fund_and_deposit(&env, &actors, 64, amount);
        env.as_contract(&actors.contract_id, || {
            let mut stats = crate::storage::get_stats(&env);
            stats.locked_total = 0;
            stats.locked_count = 0;
            crate::storage::set_stats(&env, &stats);
        });
        let c = fixtures::client(&env, &actors);
        let dest_before = fixtures::balance(&env, &actors, &actors.destination);
        assert_eq!(
            c.try_release(&actors.admin, &id),
            Err(Ok(EscrowError::Overflow))
        );
        assert_eq!(
            fixtures::balance(&env, &actors, &actors.destination),
            dest_before
        );
        // Request untouched: still Locked.
        let stored = env.as_contract(&actors.contract_id, || {
            crate::storage::get_request(&env, &id)
        });
        assert_eq!(stored.map(|r| r.status), Some(EscrowStatus::Locked));
    }

    #[test]
    fn overflow_deposit_counter_saturated() {
        // Saturated locked_count: the next deposit fails Overflow atomically.
        let (env, actors) = setup();
        fixtures::mint(&env, &actors, &actors.depositor, 1_000_000_000);
        env.as_contract(&actors.contract_id, || {
            let mut stats = crate::storage::get_stats(&env);
            stats.locked_count = u64::MAX;
            crate::storage::set_stats(&env, &stats);
        });
        let c = fixtures::client(&env, &actors);
        let res = c.try_deposit(
            &1_000_000,
            &fixtures::request_id(&env, 65),
            &actors.depositor,
            &Some(actors.destination.clone()),
            &(env.ledger().timestamp() + 3600),
        );
        assert_eq!(res, Err(Ok(EscrowError::Overflow)));
    }

    /// The whole admin surface denies strangers with typed `Unauthorized`:
    /// `set_paused`, `set_signer`, `migrate`, `transfer_admin`.
    /// (`accept_admin` is covered by `admin_rotation_two_step`.)
    #[test]
    fn admin_authz_denies_strangers() {
        use soroban_sdk::testutils::Address as _;
        use soroban_sdk::IntoVal;
        let (env, actors) = setup();
        fixtures::clear_auth_mock(&env);
        let c = fixtures::client(&env, &actors);
        let rogue = Address::generate(&env);
        let mock = |fn_name: &str, args: soroban_sdk::Vec<soroban_sdk::Val>| {
            fixtures::mock_single_call(&env, &actors.contract_id, fn_name, args, &rogue);
        };

        mock("set_paused", (rogue.clone(), true).into_val(&env));
        assert_eq!(
            c.try_set_paused(&rogue, &true),
            Err(Ok(EscrowError::Unauthorized))
        );

        let new_signer = Address::generate(&env);
        mock(
            "set_signer",
            (rogue.clone(), new_signer.clone()).into_val(&env),
        );
        assert_eq!(
            c.try_set_signer(&rogue, &new_signer),
            Err(Ok(EscrowError::Unauthorized))
        );

        let hash = BytesN::from_array(&env, &[0xab; 32]);
        mock("migrate", (rogue.clone(), hash.clone()).into_val(&env));
        assert_eq!(
            c.try_migrate(&rogue, &hash),
            Err(Ok(EscrowError::Unauthorized))
        );

        let new_admin = Address::generate(&env);
        mock(
            "transfer_admin",
            (rogue.clone(), new_admin.clone()).into_val(&env),
        );
        assert_eq!(
            c.try_transfer_admin(&rogue, &new_admin),
            Err(Ok(EscrowError::Unauthorized))
        );

        // And the signer — an operator, not an owner — is denied too.
        fixtures::mock_single_call(
            &env,
            &actors.contract_id,
            "set_paused",
            (actors.signer.clone(), true).into_val(&env),
            &actors.signer,
        );
        assert_eq!(
            c.try_set_paused(&actors.signer, &true),
            Err(Ok(EscrowError::Unauthorized))
        );
    }

    /// Decode the single event from the last invocation into
    /// `(topic, request_id_bytes, amount, actor, ledger_time)`.
    ///
    /// Must be called BEFORE any other contract invocation (even a token
    /// balance read resets the event window).
    fn decode_lifecycle_event(
        env: &Env,
        contract: &Address,
    ) -> (
        soroban_sdk::xdr::ScSymbol,
        [u8; 32],
        i128,
        soroban_sdk::xdr::ScAddress,
        u64,
    ) {
        use soroban_sdk::testutils::Events as _;
        use soroban_sdk::xdr::{ContractEventBody, ScSymbol, ScVal};
        let all = env.events().all().filter_by_contract(contract);
        let list = all.events();
        assert_eq!(list.len(), 1);
        let (topics, data) = match &list[0].body {
            ContractEventBody::V0(v0) => (&v0.topics, &v0.data),
        };
        let topic = match &topics.as_slice()[0] {
            ScVal::Symbol(s) => s.clone(),
            _ => panic!("topic[0] must be a symbol"),
        };
        let req_bytes = match &topics.as_slice()[1] {
            ScVal::Bytes(b) => {
                let s = b.as_slice();
                let mut arr = [0u8; 32];
                arr.copy_from_slice(s);
                arr
            }
            _ => panic!("topic[1] must be bytes"),
        };
        let ScVal::Map(Some(entries)) = data else {
            panic!("data must be a map")
        };
        let find = |key: &str| -> &ScVal {
            let want = ScVal::Symbol(ScSymbol::try_from(key).unwrap());
            &entries.iter().find(|e| e.key == want).unwrap().val
        };
        let amount = match find("amount") {
            ScVal::I128(parts) => (i128::from(parts.hi) << 64) | i128::from(parts.lo),
            _ => panic!("amount must be i128"),
        };
        let actor = match find("actor") {
            ScVal::Address(a) => a.clone(),
            _ => panic!("actor must be an address"),
        };
        let ledger_time = match find("ledger_time") {
            ScVal::U64(t) => *t,
            _ => panic!("ledger_time must be u64"),
        };
        (topic, req_bytes, amount, actor, ledger_time)
    }

    fn sc_symbol(name: &str) -> soroban_sdk::xdr::ScSymbol {
        soroban_sdk::xdr::ScSymbol::try_from(name).unwrap()
    }

    fn sc_address_of(addr: &Address) -> soroban_sdk::xdr::ScAddress {
        addr.into()
    }

    #[test]
    fn events_suite_deposit_payload() {
        let (env, actors) = setup();
        let amount = 42_000_000;
        let id = fund_and_deposit(&env, &actors, 70, amount);
        let (topic, req_bytes, evt_amount, actor, time) =
            decode_lifecycle_event(&env, &actors.contract_id);
        assert_eq!(topic, sc_symbol("funds_locked"));
        assert_eq!(req_bytes, id.to_array());
        assert_eq!(evt_amount, amount);
        assert_eq!(actor, sc_address_of(&actors.depositor));
        assert_eq!(time, env.ledger().timestamp());
    }

    #[test]
    fn events_suite_release_payload() {
        let (env, actors) = setup();
        let amount = 11_000_000;
        let id = fund_and_deposit(&env, &actors, 71, amount);
        let c = fixtures::client(&env, &actors);
        c.release(&actors.admin, &id);
        let (topic, req_bytes, evt_amount, actor, time) =
            decode_lifecycle_event(&env, &actors.contract_id);
        assert_eq!(topic, sc_symbol("payment_released"));
        assert_eq!(req_bytes, id.to_array());
        assert_eq!(evt_amount, amount);
        assert_eq!(actor, sc_address_of(&actors.admin));
        assert_eq!(time, env.ledger().timestamp());
    }

    #[test]
    fn events_suite_refund_payload() {
        let (env, actors) = setup();
        let amount = 9_000_000;
        let id = fund_and_deposit(&env, &actors, 72, amount);
        let c = fixtures::client(&env, &actors);
        c.refund(&actors.signer, &id);
        let (topic, req_bytes, evt_amount, actor, time) =
            decode_lifecycle_event(&env, &actors.contract_id);
        assert_eq!(topic, sc_symbol("payment_refunded"));
        assert_eq!(req_bytes, id.to_array());
        assert_eq!(evt_amount, amount);
        assert_eq!(actor, sc_address_of(&actors.signer));
        assert_eq!(time, env.ledger().timestamp());
    }

    #[test]
    fn events_suite_expire_payload() {
        use soroban_sdk::testutils::Ledger as _;
        let (env, actors) = setup();
        let amount = 7_000_000;
        let id = fund_and_deposit(&env, &actors, 73, amount);
        env.ledger().set_timestamp(env.ledger().timestamp() + 7200);
        let c = fixtures::client(&env, &actors);
        c.expire(&id);
        let (topic, req_bytes, evt_amount, actor, time) =
            decode_lifecycle_event(&env, &actors.contract_id);
        assert_eq!(topic, sc_symbol("request_expired"));
        assert_eq!(req_bytes, id.to_array());
        assert_eq!(evt_amount, amount);
        // Permissionless expire attributes the refunded depositor.
        assert_eq!(actor, sc_address_of(&actors.depositor));
        assert_eq!(time, env.ledger().timestamp());
    }

    #[test]
    fn events_suite_failed_calls_emit_nothing() {
        // Reverted invocations publish no events: absence of an event means
        // absence of mutation (indexer reliability).
        use soroban_sdk::testutils::Events as _;
        let (env, actors) = setup();
        let c = fixtures::client(&env, &actors);
        // Failed deposit (zero amount).
        let bad = c.try_deposit(
            &0,
            &fixtures::request_id(&env, 74),
            &actors.depositor,
            &Some(actors.destination.clone()),
            &(env.ledger().timestamp() + 3600),
        );
        assert_eq!(bad, Err(Ok(EscrowError::InvalidAmount)));
        assert_eq!(env.events().all().events().len(), 0);
        // Failed release (unknown id).
        let bad2 = c.try_release(&actors.admin, &fixtures::request_id(&env, 75));
        assert_eq!(bad2, Err(Ok(EscrowError::NotFound)));
        assert_eq!(env.events().all().events().len(), 0);
    }
}
