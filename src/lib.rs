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
//!
//! Rules: no `unwrap()`/`expect()` in non-test code, checked arithmetic,
//! TTL extended on every write, exactly one event per mutation.

#![no_std]

// Staged modules: each is wired by its issue (005-008, Phase B).
// The allowance is removed issue by issue as items become used.
#[allow(dead_code)]
mod admin;
mod errors;
#[allow(dead_code)]
mod events;
pub mod storage;
mod types;
#[allow(dead_code)]
mod validation;

use soroban_sdk::{contract, contractimpl, Env};

pub use errors::EscrowError;
pub use types::{EscrowRequest, EscrowStatus, SCHEMA_VERSION};

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    /// Scaffold placeholder: returns 0. Replaced by real entrypoints in Phase B.
    pub fn hello(_env: Env) -> u32 {
        0
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn hello_returns_zero() {
        let env = Env::default();
        let id = env.register(EscrowContract, ());
        let client = EscrowContractClient::new(&env, &id);
        assert_eq!(client.hello(), 0);
    }
}
