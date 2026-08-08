//! StellarRail escrow contract (scaffold).
//!
//! Issue-by-issue build per `build.md`. Full interface lands across
//! Phase B (ISSUE-011–028).

#![no_std]

use soroban_sdk::{contract, contractimpl, Env};

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
        let id = env.register_contract(None, EscrowContract);
        let client = EscrowContractClient::new(&env, &id);
        assert_eq!(client.hello(), 0);
    }
}
