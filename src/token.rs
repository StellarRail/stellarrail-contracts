//! SAC token movement helpers (XLM-only in v1).
//!
//! Design record: `docs/XLM_DESIGN.md`. Used by the settlement entrypoints
//! in Phase B (ISSUE-013/015/017/018).

use soroban_sdk::{token, Address, Env};

/// Client for the configured SAC token contract.
pub fn client<'a>(env: &'a Env, token: &'a Address) -> token::Client<'a> {
    token::Client::new(env, token)
}

/// Pull `amount` stroops from `from` into the escrow contract.
/// Requires `from.require_auth()` at the call site (deposit).
pub fn pull(env: &Env, token: &Address, from: &Address, amount: i128) {
    let vault = env.current_contract_address();
    client(env, token).transfer(from, &vault, &amount);
}

/// Push `amount` stroops from the escrow contract to `to`.
/// Called only after the request status is already terminal (checks first).
pub fn push(env: &Env, token: &Address, to: &Address, amount: i128) {
    let vault = env.current_contract_address();
    client(env, token).transfer(&vault, to, &amount);
}
