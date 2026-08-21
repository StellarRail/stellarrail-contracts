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
}
