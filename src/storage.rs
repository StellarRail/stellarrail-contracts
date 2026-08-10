//! Persistent storage schema, keys, and TTL policy.
//!
//! Keys and helpers land in ISSUE-007; this module reserves the namespace.

use soroban_sdk::{contracttype, BytesN};

/// Persistent storage keys. Singletons use unit variants; requests are
/// namespaced by id so TTL is extended per entry on every write.
#[contracttype]
pub enum DataKey {
    /// `EscrowRequest` by id.
    Request(BytesN<32>),
    /// Contract admin (`Address`).
    Admin,
    /// Authorized signer (`Address`).
    Signer,
    /// Accepted SAC XLM token (`Address`).
    Token,
    /// Emergency stop flag (`bool`).
    Paused,
    /// Ordered registry of all `request_id`s (`Vec<BytesN<32>>`) for pagination.
    AllIds,
    /// Aggregate counters (`Stats`).
    Stats,
    /// Pending admin for two-step rotation (`Address`).
    PendingAdmin,
}
