//! Admin and signer role management.
//!
//! Full implementation lands in ISSUE-011 (`initialize`) and ISSUE-025
//! (rotation). This module owns role reads/writes and the two-step
//! `transfer_admin` / `accept_admin` flow.
