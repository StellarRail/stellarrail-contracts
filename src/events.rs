//! Contract events for off-chain indexing (FR-5).
//!
//! Full `#[contractevent]` definitions land in ISSUE-008.

use soroban_sdk::{contracttype, Address, BytesN};

/// Indexer-facing payload shared by all escrow lifecycle events.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EscrowEventPayload {
    /// Which request mutated.
    pub request_id: BytesN<32>,
    /// Amount in stroops.
    pub amount: i128,
    /// Actor that triggered the mutation (or the refunded depositor for
    /// permissionless `expire`).
    pub actor: Address,
    /// `env.ledger().timestamp()` at emission.
    pub ledger_time: u64,
}
