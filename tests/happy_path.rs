//! Happy-path lifecycles against the compiled crate interface:
//! deposit→release, deposit→refund, deposit→expire.
//!
//! These run through the public contract client (like the API/CLI would),
//! complementing the unit tests in `src/`.

use escrow::{EscrowContract, EscrowContractClient, EscrowStatus};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, BytesN, Env};

struct Harness {
    env: Env,
    contract_id: Address,
    admin: Address,
    signer: Address,
    depositor: Address,
    destination: Address,
    token: Address,
}

impl Harness {
    fn new() -> Harness {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(EscrowContract, ());
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
        Harness {
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

    fn token_client(&self) -> StellarAssetClient<'_> {
        StellarAssetClient::new(&self.env, &self.token)
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
fn deposit_then_release() {
    let h = Harness::new();
    let id = h.deposit(1, 100_000_000);
    let req = h.client().release(&h.admin, &id);
    assert_eq!(req.status, EscrowStatus::Released);
    assert_eq!(h.token_client().balance(&h.destination), 100_000_000);
    assert_eq!(h.token_client().balance(&h.contract_id), 0);
}

#[test]
fn deposit_then_refund() {
    let h = Harness::new();
    let before = h.token_client().balance(&h.depositor);
    let id = h.deposit(2, 50_000_000);
    let req = h.client().refund(&h.signer, &id);
    assert_eq!(req.status, EscrowStatus::Refunded);
    assert_eq!(h.token_client().balance(&h.depositor), before);
}

#[test]
fn deposit_then_expire() {
    let h = Harness::new();
    let before = h.token_client().balance(&h.depositor);
    let id = h.deposit(3, 25_000_000);
    h.env
        .ledger()
        .set_timestamp(h.env.ledger().timestamp() + 7200);
    let req = h.client().expire(&id);
    assert_eq!(req.status, EscrowStatus::Expired);
    assert_eq!(h.token_client().balance(&h.depositor), before);
}
