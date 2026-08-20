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
// Staged: token helpers + validation consts + fixtures are wired across
// Phase B (ISSUE-012+). Removed at ISSUE-028 via full clippy sweep.
#[allow(dead_code)]
mod token;
pub mod types;
#[allow(dead_code)] // staged: validation fns used from ISSUE-012; allowance removed at ISSUE-028
mod validation;

#[cfg(test)]
#[allow(dead_code)] // staged: fixtures consumed across Phase B tests
pub(crate) mod fixtures;

use soroban_sdk::{contract, contractimpl, Address, Env};

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
}
