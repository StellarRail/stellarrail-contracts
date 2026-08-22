//! Shared test fixtures: funded Env, roles, SAC token, initialized contract.
//!
//! Happy-path tests use `mock_all_auths`. Auth-denied tests build their own
//! `mock_auths` on top of [`setup_uninitialized`] (see `auth_matrix`).

use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, BytesN, Env, IntoVal, Val, Vec as SdkVec};

use crate::EscrowContractClient;

/// Actors and ids for one test scenario.
pub struct Actors {
    pub contract_id: Address,
    pub admin: Address,
    pub signer: Address,
    pub depositor: Address,
    pub destination: Address,
    pub token: Address,
}

/// Fresh Env + registered (not yet initialized) contract + actors.
pub fn setup_uninitialized() -> (Env, Actors) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(crate::EscrowContract, ());
    let admin = Address::generate(&env);
    let signer = Address::generate(&env);
    let depositor = Address::generate(&env);
    let destination = Address::generate(&env);
    let token_issuer_admin = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(token_issuer_admin)
        .address();
    (
        env,
        Actors {
            contract_id,
            admin,
            signer,
            depositor,
            destination,
            token,
        },
    )
}

/// Env + initialized contract + minted depositor balance.
pub fn setup_initialized() -> (Env, Actors) {
    let (env, actors) = setup_uninitialized();
    env.ledger().set_timestamp(1_700_000_000);
    client(&env, &actors).initialize(&actors.admin, &actors.signer, &actors.token);
    (env, actors)
}

/// Typed client for the escrow contract.
pub fn client<'a>(env: &'a Env, actors: &Actors) -> EscrowContractClient<'a> {
    EscrowContractClient::new(env, &actors.contract_id)
}

/// Mint SAC tokens to `to` (token-admin auth is mocked).
pub fn mint(env: &Env, actors: &Actors, to: &Address, amount: i128) {
    StellarAssetClient::new(env, &actors.token).mint(to, &amount);
}

/// SAC balance of `who`.
pub fn balance(env: &Env, actors: &Actors, who: &Address) -> i128 {
    StellarAssetClient::new(env, &actors.token).balance(who)
}

/// Deterministic request id from one repeated byte.
pub fn request_id(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

/// Restrict auth to a single `caller` invoking `fn_name` with `args`.
///
/// Call `env.set_auths(&[])` first to drop `mock_all_auths`, then this helper
/// mocks exactly one authorized call. Nested token pushes from the escrow
/// contract itself need no user auth, so `sub_invokes` stays empty for
/// release/refund/pause-style entrypoints.
pub fn mock_single_call(
    env: &Env,
    contract_id: &Address,
    fn_name: &str,
    args: SdkVec<Val>,
    caller: &Address,
) {
    use soroban_sdk::testutils::{MockAuth, MockAuthInvoke};
    let invoke = MockAuthInvoke {
        contract: contract_id,
        fn_name,
        args,
        sub_invokes: &[],
    };
    env.mock_auths(&[MockAuth {
        address: caller,
        invoke: &invoke,
    }]);
}

/// Mock `release(caller, request_id)` for exactly one call.
pub fn mock_release(env: &Env, actors: &Actors, caller: &Address, id: &BytesN<32>) {
    let args = (caller.clone(), id.clone()).into_val(env);
    mock_single_call(env, &actors.contract_id, "release", args, caller);
}

/// Mock `refund(caller, request_id)` for exactly one call.
pub fn mock_refund(env: &Env, actors: &Actors, caller: &Address, id: &BytesN<32>) {
    let args = (caller.clone(), id.clone()).into_val(env);
    mock_single_call(env, &actors.contract_id, "refund", args, caller);
}

/// Drop all auth mocking (undoes `mock_all_auths`).
pub fn clear_auth_mock(env: &Env) {
    env.set_auths(&[]);
}
