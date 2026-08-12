//! Typed contract errors. See `docs/ERRORS.md` for the API mapping table.

use soroban_sdk::contracterror;

/// Machine-readable escrow failures. Discriminants are stable API.
#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EscrowError {
    /// `deposit` with an existing `request_id`, or second `initialize`.
    AlreadyExists = 1,
    /// Unknown `request_id`.
    NotFound = 2,
    /// Caller is neither admin nor signer (or not admin for admin ops).
    Unauthorized = 3,
    /// `amount <= 0` or `> MAX_AMOUNT`.
    InvalidAmount = 4,
    /// `deadline` not in `(now + MIN_DEADLINE_SECS, created + MAX_DEADLINE_SECS]`.
    InvalidDeadline = 5,
    /// Operation not allowed from the current `status`.
    InvalidState = 6,
    /// `release` after `deadline` (use `expire`).
    Expired = 7,
    /// `expire` at or before `deadline`.
    NotExpired = 8,
    /// Checked arithmetic overflow.
    Overflow = 9,
    /// Mutating entrypoint called while paused (`expire` still allowed).
    Paused = 10,
    /// `initialize` called twice.
    AlreadyInitialized = 11,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn discriminants_are_stable_api() {
        // The API maps these integers to HTTP codes (docs/ERRORS.md).
        // Never reorder or renumber.
        assert_eq!(EscrowError::AlreadyExists as u32, 1);
        assert_eq!(EscrowError::NotFound as u32, 2);
        assert_eq!(EscrowError::Unauthorized as u32, 3);
        assert_eq!(EscrowError::InvalidAmount as u32, 4);
        assert_eq!(EscrowError::InvalidDeadline as u32, 5);
        assert_eq!(EscrowError::InvalidState as u32, 6);
        assert_eq!(EscrowError::Expired as u32, 7);
        assert_eq!(EscrowError::NotExpired as u32, 8);
        assert_eq!(EscrowError::Overflow as u32, 9);
        assert_eq!(EscrowError::Paused as u32, 10);
        assert_eq!(EscrowError::AlreadyInitialized as u32, 11);
    }
}
