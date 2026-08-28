//! Input validation: amounts (stroops), deadlines, terminal-state guards.
//!
//! Full checks land in ISSUE-012/021/022; this module keeps validation pure
//! (no storage access) so it stays unit-testable without an `Env`.

use crate::errors::EscrowError;

/// Minimum escrow lifetime: 60s (lets the 60s expire drill run; the API is
/// advised to use >= 1h in production — see `docs/STORAGE.md` clock note).
pub const MIN_DEADLINE_SECS: u64 = 60;

/// Maximum escrow lifetime: 30 days.
pub const MAX_DEADLINE_SECS: u64 = 30 * 24 * 60 * 60;

/// Maximum escrow amount: 50M XLM in stroops (1 XLM = 10^7 stroops).
pub const MAX_AMOUNT: i128 = 50_000_000_000 * 10_000_000;

/// Pagination cap for `list_requests`.
pub const MAX_LIST_LIMIT: u32 = 50;

/// Amount must be a positive stroop value within the 50M XLM supply guard.
pub fn check_amount(amount: i128) -> Result<(), EscrowError> {
    if amount <= 0 || amount > MAX_AMOUNT {
        return Err(EscrowError::InvalidAmount);
    }
    Ok(())
}

/// Deadline must leave at least `MIN_DEADLINE_SECS` and at most
/// `MAX_DEADLINE_SECS` from `now` (ledger close time).
pub fn check_deadline(deadline: u64, now: u64) -> Result<(), EscrowError> {
    if deadline <= now {
        return Err(EscrowError::InvalidDeadline);
    }
    // Safe: deadline > now.
    let lifetime = deadline - now;
    if !(MIN_DEADLINE_SECS..=MAX_DEADLINE_SECS).contains(&lifetime) {
        return Err(EscrowError::InvalidDeadline);
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn amounts_accept_positive_within_cap() {
        assert!(check_amount(1).is_ok());
        assert!(check_amount(MAX_AMOUNT).is_ok());
    }

    #[test]
    fn amounts_reject_zero_negative_overflow() {
        assert_eq!(check_amount(0), Err(EscrowError::InvalidAmount));
        assert_eq!(check_amount(-1), Err(EscrowError::InvalidAmount));
        assert_eq!(
            check_amount(MAX_AMOUNT + 1),
            Err(EscrowError::InvalidAmount)
        );
        assert_eq!(check_amount(i128::MAX), Err(EscrowError::InvalidAmount));
    }

    #[test]
    fn deadlines_reject_past_and_out_of_bounds() {
        let now = 1_700_000_000;
        assert_eq!(check_deadline(now, now), Err(EscrowError::InvalidDeadline));
        assert_eq!(
            check_deadline(now - 1, now),
            Err(EscrowError::InvalidDeadline)
        );
        assert_eq!(
            check_deadline(now + MIN_DEADLINE_SECS - 1, now),
            Err(EscrowError::InvalidDeadline)
        );
        assert_eq!(
            check_deadline(now + MAX_DEADLINE_SECS + 1, now),
            Err(EscrowError::InvalidDeadline)
        );
        // A 10-year deadline is rejected.
        assert_eq!(
            check_deadline(now + 10 * 365 * 24 * 60 * 60, now),
            Err(EscrowError::InvalidDeadline)
        );
    }

    #[test]
    fn deadlines_accept_window_edges() {
        let now = 1_700_000_000;
        assert!(check_deadline(now + MIN_DEADLINE_SECS, now).is_ok());
        assert!(check_deadline(now + MAX_DEADLINE_SECS, now).is_ok());
    }
}
