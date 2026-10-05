//! Soak: 100 sequential deposits + mixed settlement locally.
//!
//! Asserts stats + pagination stay correct and the whole loop finishes in
//! well under 60s (a normal — not ignored — test).

use escrow::{EscrowContract, EscrowContractClient, EscrowStatus};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, BytesN, Env};

#[test]
fn soak_100_mixed_settles() {
    let started = std::time::Instant::now();
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
    StellarAssetClient::new(&env, &token).mint(&depositor, &1_000_000_000_000);

    let id = |i: u8| {
        BytesN::from_array(
            &env,
            &[
                i, 0xAA, 0xBB, 0xCC, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0,
            ],
        )
    };
    let deadline = env.ledger().timestamp() + 3600;

    // 100 deposits of 1M stroops.
    for i in 0u8..100u8 {
        client.deposit(
            &1_000_000,
            &id(i),
            &depositor,
            &Some(destination.clone()),
            &deadline,
        );
    }
    let full = client.get_stats();
    assert_eq!(full.locked_count, 100);
    assert_eq!(full.locked_total, 100_000_000);
    assert_eq!(full.lifetime_volume, 100_000_000);

    // Settle 50, refund 25.
    for i in 0u8..50u8 {
        client.release(&admin, &id(i));
    }
    for i in 50u8..75u8 {
        client.refund(&signer, &id(i));
    }
    // Expire the last 25 past the deadline.
    env.ledger().set_timestamp(deadline + 1);
    for i in 75u8..100u8 {
        client.expire(&id(i));
    }

    // Stats drained to zero-locked, volume preserved.
    let end = client.get_stats();
    assert_eq!(end.locked_count, 0);
    assert_eq!(end.locked_total, 0);
    assert_eq!(end.released_count, 50);
    assert_eq!(end.lifetime_volume, 100_000_000);

    // Pagination still walks the full 100 across two capped pages.
    assert_eq!(client.list_requests(&None, &0, &50).len(), 50);
    assert_eq!(client.list_requests(&None, &50, &50).len(), 50);
    let released = client.list_requests(&Some(EscrowStatus::Released), &0, &50);
    assert_eq!(released.len(), 50);

    let elapsed = started.elapsed();
    assert!(
        elapsed.as_secs() < 60,
        "soak took {elapsed:?}, budget is 60s"
    );
}
