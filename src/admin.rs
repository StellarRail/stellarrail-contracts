//! Admin and signer role management.
//!
//! All privileged entrypoints authenticate an explicit `caller` address
//! (`caller.require_auth()` + role comparison) so failures surface the typed
//! `Unauthorized` code instead of a host auth trap. Rationale: Soroban
//! `require_auth` over fixed addresses can only express AND, not the
//! admin-OR-signer rule — see `docs/adr/003-explicit-caller-param.md`.

use soroban_sdk::{Address, BytesN, ContractExecutable, Env};

use crate::errors::EscrowError;
use crate::events;
use crate::storage;

/// Fail unless the contract was initialized.
fn initialized_admin(env: &Env) -> Result<Address, EscrowError> {
    storage::get_admin(env).ok_or(EscrowError::NotFound)
}

/// Fail unless `caller` authed and is the admin.
///
/// # Errors
///
/// Returns `NotFound` (uninitialized) or `Unauthorized`.
pub fn require_admin(env: &Env, caller: &Address) -> Result<Address, EscrowError> {
    caller.require_auth();
    let admin = initialized_admin(env)?;
    if *caller != admin {
        return Err(EscrowError::Unauthorized);
    }
    Ok(admin)
}

/// Fail unless `caller` authed and is the admin or the signer.
///
/// # Errors
///
/// Returns `NotFound` (uninitialized) or `Unauthorized`.
pub fn require_admin_or_signer(env: &Env, caller: &Address) -> Result<Address, EscrowError> {
    caller.require_auth();
    let admin = initialized_admin(env)?;
    let signer = storage::get_signer(env).ok_or(EscrowError::NotFound)?;
    if *caller != admin && *caller != signer {
        return Err(EscrowError::Unauthorized);
    }
    Ok(caller.clone())
}

/// One-time setup. `admin` authorizes; double init fails `AlreadyExists`.
///
/// # Errors
///
/// Returns `AlreadyExists` on second call.
pub fn do_initialize(
    env: &Env,
    admin: &Address,
    signer: &Address,
    token: &Address,
) -> Result<(), EscrowError> {
    admin.require_auth();
    if storage::get_admin(env).is_some() {
        return Err(EscrowError::AlreadyExists);
    }
    storage::set_admin(env, admin);
    storage::set_signer(env, signer);
    storage::set_token(env, token);
    storage::set_paused(env, false);
    storage::set_stats(env, &storage::Stats::zeroed());
    events::emit_initialized(env, admin, signer, token);
    Ok(())
}

/// Flip the emergency stop (admin only). `expire` is unaffected.
///
/// # Errors
///
/// Returns `NotFound` (uninitialized) or `Unauthorized`.
pub fn do_set_paused(env: &Env, caller: &Address, paused: bool) -> Result<(), EscrowError> {
    require_admin(env, caller)?;
    storage::set_paused(env, paused);
    events::emit_pause_toggled(env, caller, paused);
    Ok(())
}

/// Stage a new admin (admin only). Takes effect on `accept_admin`.
///
/// # Errors
///
/// Returns `NotFound` (uninitialized) or `Unauthorized`.
pub fn do_transfer_admin(
    env: &Env,
    caller: &Address,
    new_admin: &Address,
) -> Result<(), EscrowError> {
    require_admin(env, caller)?;
    storage::set_pending_admin(env, new_admin);
    events::emit_admin_transfer_initiated(env, caller, new_admin);
    Ok(())
}

/// Complete rotation: the staged admin authorizes acceptance.
///
/// # Errors
///
/// Returns `NotFound` (no rotation staged) or `Unauthorized` (caller is not
/// the staged admin).
pub fn do_accept_admin(env: &Env, caller: &Address) -> Result<(), EscrowError> {
    caller.require_auth();
    let pending = storage::get_pending_admin(env).ok_or(EscrowError::NotFound)?;
    if *caller != pending {
        return Err(EscrowError::Unauthorized);
    }
    storage::set_admin(env, &pending);
    storage::clear_pending_admin(env);
    events::emit_admin_transfer_accepted(env, &pending);
    Ok(())
}

/// Rotate the signer (admin only). The old signer is invalid immediately.
///
/// # Errors
///
/// Returns `NotFound` (uninitialized) or `Unauthorized`.
pub fn do_set_signer(env: &Env, caller: &Address, new_signer: &Address) -> Result<(), EscrowError> {
    require_admin(env, caller)?;
    let old = storage::get_signer(env).ok_or(EscrowError::NotFound)?;
    storage::set_signer(env, new_signer);
    events::emit_signer_updated(env, caller, &old, new_signer);
    Ok(())
}

/// Upgrade the contract WASM (admin only).
///
/// # Errors
///
/// Returns `NotFound` (uninitialized) or `Unauthorized`.
pub fn do_migrate(
    env: &Env,
    caller: &Address,
    new_wasm_hash: &BytesN<32>,
) -> Result<(), EscrowError> {
    require_admin(env, caller)?;
    env.deployer()
        .update_current_contract(ContractExecutable::Wasm(new_wasm_hash.clone()));
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fixtures::{self, Actors};
    use soroban_sdk::testutils::{Address as _, Events as _};

    fn setup() -> (Env, Actors) {
        fixtures::setup_uninitialized()
    }

    #[test]
    fn initialize_stores_roles_and_token() {
        let (env, actors) = setup();
        let client = crate::EscrowContractClient::new(&env, &actors.contract_id);
        client.initialize(&actors.admin, &actors.signer, &actors.token);

        // Exactly one Initialized event (read before any further invocation,
        // which would reset the event window).
        let events = env.events().all().filter_by_contract(&actors.contract_id);
        assert_eq!(events.events().len(), 1);

        env.as_contract(&actors.contract_id, || {
            assert_eq!(storage::get_admin(&env), Some(actors.admin.clone()));
            assert_eq!(storage::get_signer(&env), Some(actors.signer.clone()));
            assert_eq!(storage::get_token(&env), Some(actors.token.clone()));
            assert!(!storage::is_paused(&env));
        });
    }

    #[test]
    fn initialize_twice_fails() {
        let (env, actors) = setup();
        let client = crate::EscrowContractClient::new(&env, &actors.contract_id);
        client.initialize(&actors.admin, &actors.signer, &actors.token);
        let second = client.try_initialize(&actors.admin, &actors.signer, &actors.token);
        // Contract-level errors surface in the outer Err (convertible to EscrowError).
        assert_eq!(second, Err(Ok(EscrowError::AlreadyExists)));
    }

    #[test]
    fn initialize_requires_admin_auth() {
        // No mocked auths: the host must reject an unauthenticated initialize.
        let env = Env::default();
        let contract_id = env.register(crate::EscrowContract, ());
        let client = crate::EscrowContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let signer = Address::generate(&env);
        let token = Address::generate(&env);
        let res = client.try_initialize(&admin, &signer, &token);
        assert!(res.is_err(), "unauthenticated initialize must trap");
    }
}
