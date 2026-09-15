//! Persistent storage schema, keys, TTL policy, and counters.
//!
//! Every write goes through a helper below so TTL extension cannot be
//! forgotten. Reads never mutate TTL (views stay read-only).
//!
//! Ledger timing: Stellar closes a ledger ~every 5s → ~`17_280` ledgers/day.

use soroban_sdk::{contracttype, Address, BytesN, Env, Vec};

use crate::types::EscrowRequest;

/// Extend persistent entries whose remaining TTL is below this (~7 days).
/// Chosen so a weekly keeper (or any user interaction) keeps live escrows
/// pinned without constant writes.
pub const PERSISTENT_TTL_THRESHOLD: u32 = 120_960;

/// TTL that writes extend entries to (~30 days, the maximum useful horizon:
/// it matches `MAX_DEADLINE_SECS`, so no live escrow can be evicted while
/// its deadline is still in the future as long as each mutation extends).
pub const PERSISTENT_EXTEND_TO: u32 = 518_400;

/// Persistent storage keys. Singletons use unit variants; requests are
/// namespaced by id so TTL is extended per entry on every write.
#[contracttype]
pub enum DataKey {
    /// `EscrowRequest` by id.
    Request(BytesN<32>),
    /// Contract admin (`Address`).
    Admin,
    /// Authorized signer (`Address`).
    Signer,
    /// Accepted SAC XLM token (`Address`).
    Token,
    /// Emergency stop flag (`bool`).
    Paused,
    /// Ordered registry of all `request_id`s (`Vec<BytesN<32>>`) for pagination.
    AllIds,
    /// Aggregate counters (`Stats`).
    Stats,
    /// Pending admin for two-step rotation (`Address`).
    PendingAdmin,
}

/// O(1) aggregates maintained atomically with request writes (ISSUE-040).
/// `locked_total`/`lifetime_volume` are stroops (`i128`), always
/// checked-arithmetic guarded at the call site.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stats {
    /// Requests currently `Locked`.
    pub locked_count: u64,
    /// Sum of amounts currently `Locked`.
    pub locked_total: i128,
    /// Requests terminally `Released` (cumulative).
    pub released_count: u64,
    /// Sum of all deposited amounts (cumulative, never decreases).
    pub lifetime_volume: i128,
}

impl Stats {
    /// Zero counters for `initialize`.
    #[must_use]
    pub fn zeroed() -> Stats {
        Stats {
            locked_count: 0,
            locked_total: 0,
            released_count: 0,
            lifetime_volume: 0,
        }
    }
}

/// Extend the TTL of a persistent key to the policy window.
fn extend(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_TTL_THRESHOLD, PERSISTENT_EXTEND_TO);
}

/// Read a request without touching TTL (views stay read-only).
#[must_use]
pub fn get_request(env: &Env, request_id: &BytesN<32>) -> Option<EscrowRequest> {
    env.storage()
        .persistent()
        .get(&DataKey::Request(request_id.clone()))
}

/// Write a request and extend its TTL. Always pair with event emission.
pub fn set_request(env: &Env, request: &EscrowRequest) {
    let key = DataKey::Request(request.request_id.clone());
    env.storage().persistent().set(&key, request);
    extend(env, &key);
}

/// Record a new id in the pagination registry (idempotent) and extend TTL.
pub fn track_request_id(env: &Env, request_id: &BytesN<32>) {
    let key = DataKey::AllIds;
    let mut ids: Vec<BytesN<32>> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env));
    if !contains_id(&ids, request_id) {
        ids.push_back(request_id.clone());
        env.storage().persistent().set(&key, &ids);
    }
    extend(env, &key);
}

fn contains_id(ids: &Vec<BytesN<32>>, target: &BytesN<32>) -> bool {
    let mut found = false;
    for id in ids.iter() {
        if id == *target {
            found = true;
            break;
        }
    }
    found
}

/// Paginated id registry (raw; filtering happens in `list_requests`).
#[must_use]
pub fn get_all_ids(env: &Env) -> Vec<BytesN<32>> {
    env.storage()
        .persistent()
        .get(&DataKey::AllIds)
        .unwrap_or(Vec::new(env))
}

fn get_addr(env: &Env, key: &DataKey) -> Option<Address> {
    env.storage().persistent().get(key)
}

fn set_addr(env: &Env, key: DataKey, value: &Address) {
    env.storage().persistent().set(&key, value);
    extend(env, &key);
}

/// Contract admin (set once by `initialize`, moved by two-step rotation).
#[must_use]
pub fn get_admin(env: &Env) -> Option<Address> {
    get_addr(env, &DataKey::Admin)
}

/// Write admin and extend TTL.
pub fn set_admin(env: &Env, admin: &Address) {
    set_addr(env, DataKey::Admin, admin);
}

/// Authorized signer (second release/refund authority).
#[must_use]
pub fn get_signer(env: &Env) -> Option<Address> {
    get_addr(env, &DataKey::Signer)
}

/// Write signer and extend TTL.
pub fn set_signer(env: &Env, signer: &Address) {
    set_addr(env, DataKey::Signer, signer);
}

/// Accepted SAC XLM token contract.
#[must_use]
pub fn get_token(env: &Env) -> Option<Address> {
    get_addr(env, &DataKey::Token)
}

/// Write token id and extend TTL.
pub fn set_token(env: &Env, token: &Address) {
    set_addr(env, DataKey::Token, token);
}

/// Pending admin for two-step rotation (`None` when no rotation in flight).
#[must_use]
pub fn get_pending_admin(env: &Env) -> Option<Address> {
    get_addr(env, &DataKey::PendingAdmin)
}

/// Stage a pending admin and extend TTL.
pub fn set_pending_admin(env: &Env, pending: &Address) {
    set_addr(env, DataKey::PendingAdmin, pending);
}

/// Clear a finished/cancelled rotation.
pub fn clear_pending_admin(env: &Env) {
    env.storage().persistent().remove(&DataKey::PendingAdmin);
}

/// Emergency stop flag. `true` blocks deposit/release/refund; `expire`
/// stays open so funds remain rescuable.
#[must_use]
pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

/// Write the pause flag and extend TTL.
pub fn set_paused(env: &Env, paused: bool) {
    let key = DataKey::Paused;
    env.storage().persistent().set(&key, &paused);
    extend(env, &key);
}

/// Aggregate counters (zeroed at `initialize`).
#[must_use]
pub fn get_stats(env: &Env) -> Stats {
    env.storage()
        .persistent()
        .get(&DataKey::Stats)
        .unwrap_or(Stats::zeroed())
}

/// Write counters and extend TTL. Callers update fields with checked
/// arithmetic before calling.
pub fn set_stats(env: &Env, stats: &Stats) {
    let key = DataKey::Stats;
    env.storage().persistent().set(&key, stats);
    extend(env, &key);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::types::EscrowStatus;
    use crate::EscrowContract;
    use soroban_sdk::testutils::storage::Persistent as _;
    use soroban_sdk::testutils::{Address as _, Ledger};

    fn setup() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let id = env.register(EscrowContract, ());
        (env, id)
    }

    fn sample(env: &Env) -> EscrowRequest {
        EscrowRequest {
            request_id: BytesN::from_array(env, &[9u8; 32]),
            depositor: Address::generate(env),
            destination: Some(Address::generate(env)),
            amount: 10_000_000,
            deadline: 2_000_000,
            status: EscrowStatus::Locked,
            created_at: 1_000,
            updated_at: 1_000,
        }
    }

    #[test]
    fn writes_extend_ttl_to_policy_window() {
        let (env, id) = setup();
        env.ledger().set_sequence_number(1_000_000);
        let req = sample(&env);
        env.as_contract(&id, || {
            set_request(&env, &req);
            let ttl = env
                .storage()
                .persistent()
                .get_ttl(&DataKey::Request(req.request_id.clone()));
            assert_eq!(ttl, PERSISTENT_EXTEND_TO);
        });
    }

    #[test]
    fn entry_survives_to_end_of_extended_window() {
        let (env, id) = setup();
        env.ledger().set_sequence_number(500_000);
        let req = sample(&env);
        env.as_contract(&id, || set_request(&env, &req));
        // Jump to the last ledger of the extended window: without our
        // extend-on-write the entry would long be gone.
        env.ledger()
            .set_sequence_number(500_000 + PERSISTENT_EXTEND_TO - 1);
        let found: bool = env.as_contract(&id, || get_request(&env, &req.request_id).is_some());
        assert!(found);
    }

    #[test]
    fn singleton_writes_extend_ttl() {
        let (env, id) = setup();
        env.ledger().set_sequence_number(42);
        let admin = Address::generate(&env);
        env.as_contract(&id, || {
            set_admin(&env, &admin);
            set_paused(&env, true);
            assert_eq!(
                env.storage().persistent().get_ttl(&DataKey::Admin),
                PERSISTENT_EXTEND_TO
            );
            assert_eq!(
                env.storage().persistent().get_ttl(&DataKey::Paused),
                PERSISTENT_EXTEND_TO
            );
            assert!(is_paused(&env));
            assert_eq!(get_admin(&env), Some(admin));
        });
    }

    #[test]
    fn id_registry_is_idempotent() {
        let (env, id) = setup();
        let req = sample(&env);
        env.as_contract(&id, || {
            track_request_id(&env, &req.request_id);
            track_request_id(&env, &req.request_id);
            assert_eq!(get_all_ids(&env).len(), 1);
        });
    }

    #[test]
    fn stats_default_to_zero() {
        let (env, id) = setup();
        let stats: Stats = env.as_contract(&id, || get_stats(&env));
        assert_eq!(stats, Stats::zeroed());
    }
}
