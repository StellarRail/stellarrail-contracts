//! Adversarial-input fuzz: 10 000 pseudo-random deposits must only ever
//! succeed or fail with a typed `EscrowError` — never a host trap.
//!
//! Deterministic xorshift64 PRNG (fixed seed) so failures reproduce exactly.
//! The id space is deliberately tiny (500 ids) to force `AlreadyExists`
//! collisions; amounts/deadlines sweep all validation boundaries.

use escrow::validation::{MAX_AMOUNT, MAX_DEADLINE_SECS, MIN_DEADLINE_SECS};
use escrow::{EscrowContract, EscrowContractClient, EscrowError};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{Address, BytesN, Env};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

#[test]
fn fuzz_deposit_inputs() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EscrowContract, ());
    let depositor = Address::generate(&env);
    let destination = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    let now = 1_700_000_000u64;
    env.ledger().set_timestamp(now);
    let client = EscrowContractClient::new(&env, &contract_id);
    client.initialize(&Address::generate(&env), &Address::generate(&env), &token);
    // Funded far beyond the maximum possible drain (500 distinct ids ×
    // MAX_AMOUNT each ≈ 2.5e20), so a pull can never trap on balance.
    StellarAssetClient::new(&env, &token).mint(&depositor, &1_000_000_000_000_000_000_000_000_000);

    let mut rng = Rng(0x9E3779B97F4A7C15);
    let mut ok_count = 0u32;
    let mut already_exists = 0u32;
    let mut invalid_amount = 0u32;
    let mut invalid_deadline = 0u32;

    for _ in 0..10_000u32 {
        let r = rng.next();
        let amount: i128 = match r % 8 {
            0 => 0,
            1 => -(((r >> 3) % 1_000_000) as i128) - 1,
            2 => 1,
            3 => MAX_AMOUNT,
            4 => MAX_AMOUNT + 1,
            5 => i128::MAX,
            6 => (((r >> 3) % 1_000_000_000_000) as i128) + 1,
            _ => (((r >> 3) % (MAX_AMOUNT as u64 * 2)) as i128) + 1,
        };
        let deadline: u64 = match (r >> 5) % 6 {
            0 => now - (r % 100_000) - 1,
            1 => now,
            2 => now + (r % MIN_DEADLINE_SECS),
            3 => now + MIN_DEADLINE_SECS + (r % 3500),
            4 => now + MAX_DEADLINE_SECS + 1 + (r % 100_000),
            _ => now + 10 * 365 * 24 * 60 * 60,
        };
        // Tiny id space (500 values) forces AlreadyExists collisions.
        let id_byte = ((r >> 11) % 500) as u8;
        let id = BytesN::from_array(&env, &[id_byte; 32]);
        let dest = if r.is_multiple_of(17) {
            None
        } else {
            Some(destination.clone())
        };

        let res = client.try_deposit(&amount, &id, &depositor, &dest, &deadline);
        match res {
            Ok(Ok(_)) => ok_count += 1,
            Err(Ok(EscrowError::AlreadyExists)) => already_exists += 1,
            Err(Ok(EscrowError::InvalidAmount)) => invalid_amount += 1,
            Err(Ok(EscrowError::InvalidDeadline)) => invalid_deadline += 1,
            // Anything else — a host trap or an unexpected code — is a bug.
            other => panic!("unexpected deposit outcome: {other:?}"),
        }
    }

    // Sanity: the fuzzer actually explored every class.
    assert!(ok_count > 100, "ok={ok_count}");
    assert!(already_exists > 100, "collisions={already_exists}");
    assert!(invalid_amount > 100, "amount={invalid_amount}");
    assert!(invalid_deadline > 100, "deadline={invalid_deadline}");
}
