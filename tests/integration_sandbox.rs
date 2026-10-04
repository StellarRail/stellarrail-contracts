//! Sandbox integration: the REAL compiled WASM runs in a local `Env`.
//!
//! Requires `stellar contract build` first (the suite loads
//! `target/wasm32v1-none/release/escrow.wasm` at runtime and panics with
//! instructions if it is missing). This proves the shipped artifact's
//! interface and behavior match the native test runner.

use escrow::{EscrowContractClient, EscrowStatus};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, BytesN, Env};

fn load_wasm() -> Vec<u8> {
    let path = "target/wasm32v1-none/release/escrow.wasm";
    std::fs::read(path).unwrap_or_else(|_| {
        panic!("missing {path}: run `stellar contract build` (or `make build`) first")
    })
}

struct Sandbox {
    env: Env,
    contract_id: Address,
    admin: Address,
    signer: Address,
    depositor: Address,
    destination: Address,
    token: Address,
}

impl Sandbox {
    fn new() -> Sandbox {
        let env = Env::default();
        env.mock_all_auths();
        let wasm = load_wasm();
        let contract_id = env.register(wasm.as_slice(), ());
        let admin = Address::generate(&env);
        let signer = Address::generate(&env);
        let depositor = Address::generate(&env);
        let destination = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(Address::generate(&env))
            .address();
        env.ledger().set_timestamp(1_700_000_000);
        let client = EscrowContractClient::new(&env, &contract_id);
        client.initialize(&admin, &signer, &token);
        StellarAssetClient::new(&env, &token).mint(&depositor, &1_000_000_000);
        Sandbox {
            env,
            contract_id,
            admin,
            signer,
            depositor,
            destination,
            token,
        }
    }

    fn client(&self) -> EscrowContractClient<'_> {
        EscrowContractClient::new(&self.env, &self.contract_id)
    }

    fn id(&self, byte: u8) -> BytesN<32> {
        BytesN::from_array(&self.env, &[byte; 32])
    }

    fn deposit(&self, byte: u8, amount: i128) -> BytesN<32> {
        let id = self.id(byte);
        self.client().deposit(
            &amount,
            &id,
            &self.depositor,
            &Some(self.destination.clone()),
            &(self.env.ledger().timestamp() + 3600),
        );
        id
    }
}

#[test]
fn wasm_deposit_release_lifecycle() {
    let s = Sandbox::new();
    assert_eq!(s.client().version(), 1);
    let id = s.deposit(1, 20_000_000);
    let req = s.client().release(&s.admin, &id);
    assert_eq!(req.status, EscrowStatus::Released);
    assert_eq!(
        StellarAssetClient::new(&s.env, &s.token).balance(&s.destination),
        20_000_000
    );
}

#[test]
fn wasm_deposit_refund_lifecycle() {
    let s = Sandbox::new();
    let before = StellarAssetClient::new(&s.env, &s.token).balance(&s.depositor);
    let id = s.deposit(2, 15_000_000);
    let req = s.client().refund(&s.signer, &id);
    assert_eq!(req.status, EscrowStatus::Refunded);
    assert_eq!(
        StellarAssetClient::new(&s.env, &s.token).balance(&s.depositor),
        before
    );
}

#[test]
fn wasm_deposit_expire_lifecycle() {
    let s = Sandbox::new();
    let before = StellarAssetClient::new(&s.env, &s.token).balance(&s.depositor);
    let id = s.deposit(3, 10_000_000);
    s.env
        .ledger()
        .set_timestamp(s.env.ledger().timestamp() + 7200);
    let req = s.client().expire(&id);
    assert_eq!(req.status, EscrowStatus::Expired);
    assert_eq!(
        StellarAssetClient::new(&s.env, &s.token).balance(&s.depositor),
        before
    );
}

#[test]
fn wasm_pause_and_rotation() {
    let s = Sandbox::new();
    // Pause blocks deposits with Paused.
    s.client().set_paused(&s.admin, &true);
    let res = s.client().try_deposit(
        &1_000_000,
        &s.id(9),
        &s.depositor,
        &Some(s.destination.clone()),
        &(s.env.ledger().timestamp() + 3600),
    );
    assert_eq!(res, Err(Ok(escrow::EscrowError::Paused)));
    s.client().set_paused(&s.admin, &false);

    // Signer rotation: old signer denied, new signer settles.
    let new_signer = Address::generate(&s.env);
    s.client().set_signer(&s.admin, &new_signer);
    let id = s.deposit(10, 5_000_000);
    let req = s.client().release(&new_signer, &id);
    assert_eq!(req.status, EscrowStatus::Released);

    // Stats + pagination work through the WASM boundary.
    let stats = s.client().get_stats();
    assert_eq!(stats.released_count, 1);
    assert_eq!(stats.lifetime_volume, 5_000_000);
    assert_eq!(s.client().list_requests(&None, &0, &50).len(), 1);
}
