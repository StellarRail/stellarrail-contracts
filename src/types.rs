//! Canonical escrow types: [`EscrowStatus`], [`EscrowRequest`].
//!
//! Schema version: [`SCHEMA_VERSION`]. Storage layout is documented in
//! `docs/STORAGE.md`.

use soroban_sdk::{contracttype, Address, BytesN};

/// Storage schema version. Bump on any breaking state change; surfaced via
/// `version()` and checked by the release workflow.
pub const SCHEMA_VERSION: u32 = 1;

/// Lifecycle status of an escrow request.
///
/// ```text
/// Created -> Locked -> Released   (terminal)
///                \--> Refunded   (terminal, rejection path)
///                \--> Expired    (terminal, permissionless cleanup)
/// Locked -> Failed                (terminal, reserved for v1.x settlement faults)
/// ```
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EscrowStatus {
    /// Reserved: request announced but not yet funded. `deposit` writes
    /// `Locked` directly; kept for forward-compatible indexing.
    Created = 0,
    /// Funds locked in the contract; awaiting `release` / `refund` / `expire`.
    Locked = 1,
    /// Funds paid out to `destination`. Terminal.
    Released = 2,
    /// Funds returned to `depositor` via `refund`. Terminal.
    Refunded = 3,
    /// Past `deadline`; funds returned to `depositor` via `expire`. Terminal.
    Expired = 4,
    /// Reserved: settlement fault marker for future versions. Terminal.
    Failed = 5,
}

/// A single escrow request.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EscrowRequest {
    /// Caller-supplied unique id (e.g. UUID bytes). Uniqueness enforced.
    pub request_id: BytesN<32>,
    /// Funder; authorizes `deposit` and receives `refund`/`expire` payouts.
    pub depositor: Address,
    /// Payout target for `release`. `None` is rejected at deposit time in v1
    /// (kept `Option` for forward-compatible API evolution).
    pub destination: Option<Address>,
    /// Locked amount in stroops (`i128`). Always `> 0`, `<= MAX_AMOUNT`.
    pub amount: i128,
    /// Unix seconds after which anyone may call `expire`.
    pub deadline: u64,
    /// Current lifecycle status.
    pub status: EscrowStatus,
    /// Ledger timestamp at `deposit`.
    pub created_at: u64,
    /// Ledger timestamp of the last mutation.
    pub updated_at: u64,
}
