//! Input validation: amounts (stroops), deadlines, terminal-state guards.
//!
//! Full checks land in ISSUE-012/021/022; this module keeps validation pure
//! (no storage access) so it stays unit-testable without an `Env`.

/// Minimum escrow lifetime: 60s (lets the 60s expire drill run; the API is
/// advised to use >= 1h in production — see `docs/STORAGE.md` clock note).
pub const MIN_DEADLINE_SECS: u64 = 60;

/// Maximum escrow lifetime: 30 days.
pub const MAX_DEADLINE_SECS: u64 = 30 * 24 * 60 * 60;

/// Maximum escrow amount: 50M XLM in stroops (1 XLM = 10^7 stroops).
pub const MAX_AMOUNT: i128 = 50_000_000_000 * 10_000_000;

/// Pagination cap for `list_requests`.
pub const MAX_LIST_LIMIT: u32 = 50;
