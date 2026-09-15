//! Contract events for off-chain indexing (FR-5).
//!
//! Every state mutation emits exactly one event. Indexer contract is
//! `docs/EVENTS.md` (topics, fields, example JSON).

use soroban_sdk::{contractevent, Address, BytesN, Env};

/// Funds locked by `deposit`. Indexer: creates the request row.
#[contractevent]
#[derive(Clone)]
pub struct FundsLocked {
    #[topic]
    pub request_id: BytesN<32>,
    pub amount: i128,
    pub actor: Address,
    pub ledger_time: u64,
}

/// Funds paid to `destination` by `release`. Indexer: marks released.
#[contractevent]
#[derive(Clone)]
pub struct PaymentReleased {
    #[topic]
    pub request_id: BytesN<32>,
    pub amount: i128,
    pub actor: Address,
    pub ledger_time: u64,
}

/// Funds returned to `depositor` by `refund`. Indexer: marks refunded.
#[contractevent]
#[derive(Clone)]
pub struct PaymentRefunded {
    #[topic]
    pub request_id: BytesN<32>,
    pub amount: i128,
    pub actor: Address,
    pub ledger_time: u64,
}

/// Past-due funds returned to `depositor` by permissionless `expire`.
/// Indexer: marks expired.
#[contractevent]
#[derive(Clone)]
pub struct RequestExpired {
    #[topic]
    pub request_id: BytesN<32>,
    pub amount: i128,
    pub actor: Address,
    pub ledger_time: u64,
}

/// One-time setup marker. Indexer: records admin/signer/token.
#[contractevent]
#[derive(Clone)]
pub struct Initialized {
    #[topic]
    pub admin: Address,
    pub signer: Address,
    pub token: Address,
}

/// Emit `FundsLocked` for a deposit.
pub fn emit_funds_locked(env: &Env, request_id: &BytesN<32>, amount: i128, actor: &Address) {
    FundsLocked {
        request_id: request_id.clone(),
        amount,
        actor: actor.clone(),
        ledger_time: env.ledger().timestamp(),
    }
    .publish(env);
}

/// Emit `PaymentReleased` for a release.
pub fn emit_payment_released(env: &Env, request_id: &BytesN<32>, amount: i128, actor: &Address) {
    PaymentReleased {
        request_id: request_id.clone(),
        amount,
        actor: actor.clone(),
        ledger_time: env.ledger().timestamp(),
    }
    .publish(env);
}

/// Emit `PaymentRefunded` for a refund.
pub fn emit_payment_refunded(env: &Env, request_id: &BytesN<32>, amount: i128, actor: &Address) {
    PaymentRefunded {
        request_id: request_id.clone(),
        amount,
        actor: actor.clone(),
        ledger_time: env.ledger().timestamp(),
    }
    .publish(env);
}

/// Emit `RequestExpired` for an expiry.
pub fn emit_request_expired(env: &Env, request_id: &BytesN<32>, amount: i128, actor: &Address) {
    RequestExpired {
        request_id: request_id.clone(),
        amount,
        actor: actor.clone(),
        ledger_time: env.ledger().timestamp(),
    }
    .publish(env);
}

/// Emit `Initialized` for contract setup.
pub fn emit_initialized(env: &Env, admin: &Address, signer: &Address, token: &Address) {
    Initialized {
        admin: admin.clone(),
        signer: signer.clone(),
        token: token.clone(),
    }
    .publish(env);
}

/// Emergency stop engaged. Indexer: halt settlement expectations
/// (`expire` remains possible).
#[contractevent]
#[derive(Clone)]
pub struct Paused {
    #[topic]
    pub actor: Address,
}

/// Emergency stop lifted.
#[contractevent]
#[derive(Clone)]
pub struct Unpaused {
    #[topic]
    pub actor: Address,
}

/// Signer rotated by admin. Old signer is invalid immediately.
#[contractevent]
#[derive(Clone)]
pub struct SignerUpdated {
    #[topic]
    pub new_signer: Address,
    pub old_signer: Address,
    pub actor: Address,
}

/// First half of two-step admin rotation.
#[contractevent]
#[derive(Clone)]
pub struct AdminTransferInitiated {
    #[topic]
    pub new_admin: Address,
    pub actor: Address,
}

/// Rotation completed by the staged admin.
#[contractevent]
#[derive(Clone)]
pub struct AdminTransferAccepted {
    #[topic]
    pub new_admin: Address,
}

/// Emit `Paused` / `Unpaused`.
pub fn emit_pause_toggled(env: &Env, actor: &Address, paused: bool) {
    if paused {
        Paused {
            actor: actor.clone(),
        }
        .publish(env);
    } else {
        Unpaused {
            actor: actor.clone(),
        }
        .publish(env);
    }
}

/// Emit `SignerUpdated`.
pub fn emit_signer_updated(env: &Env, actor: &Address, old_signer: &Address, new_signer: &Address) {
    SignerUpdated {
        new_signer: new_signer.clone(),
        old_signer: old_signer.clone(),
        actor: actor.clone(),
    }
    .publish(env);
}

/// Emit `AdminTransferInitiated`.
pub fn emit_admin_transfer_initiated(env: &Env, actor: &Address, new_admin: &Address) {
    AdminTransferInitiated {
        new_admin: new_admin.clone(),
        actor: actor.clone(),
    }
    .publish(env);
}

/// Emit `AdminTransferAccepted`.
pub fn emit_admin_transfer_accepted(env: &Env, new_admin: &Address) {
    AdminTransferAccepted {
        new_admin: new_admin.clone(),
    }
    .publish(env);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::EscrowContract;
    use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
    use soroban_sdk::xdr::{ContractEventBody, ScSymbol, ScVal};

    fn setup() -> (Env, Address) {
        let env = Env::default();
        let id = env.register(EscrowContract, ());
        (env, id)
    }

    fn topic0(body: &ContractEventBody) -> ScSymbol {
        match body {
            ContractEventBody::V0(v0) => match &v0.topics.as_slice()[0] {
                ScVal::Symbol(s) => s.clone(),
                _ => panic!("unexpected topic type"),
            },
        }
    }

    fn sym(name: &str) -> ScSymbol {
        ScSymbol::try_from(name).unwrap_or_else(|()| panic!("bad symbol"))
    }

    #[test]
    fn funds_locked_publishes_typed_event() {
        let (env, id) = setup();
        env.ledger().set_timestamp(1_777_777);
        let request_id = BytesN::from_array(&env, &[1u8; 32]);
        let actor = Address::generate(&env);
        env.as_contract(&id, || {
            emit_funds_locked(&env, &request_id, 5_000_000, &actor);
        });

        let events = env.events().all().filter_by_contract(&id);
        let list = events.events();
        assert_eq!(list.len(), 1);
        assert_eq!(topic0(&list[0].body), sym("funds_locked"));
    }

    #[test]
    fn every_lifecycle_event_has_distinct_topic() {
        let (env, id) = setup();
        let request_id = BytesN::from_array(&env, &[2u8; 32]);
        let actor = Address::generate(&env);
        let admin = Address::generate(&env);
        let signer = Address::generate(&env);
        let token = Address::generate(&env);
        env.as_contract(&id, || {
            emit_funds_locked(&env, &request_id, 1, &actor);
            emit_payment_released(&env, &request_id, 1, &actor);
            emit_payment_refunded(&env, &request_id, 1, &actor);
            emit_request_expired(&env, &request_id, 1, &actor);
            emit_initialized(&env, &admin, &signer, &token);
        });

        let events = env.events().all().filter_by_contract(&id);
        let list = events.events();
        assert_eq!(list.len(), 5);
        assert_eq!(topic0(&list[0].body), sym("funds_locked"));
        assert_eq!(topic0(&list[1].body), sym("payment_released"));
        assert_eq!(topic0(&list[2].body), sym("payment_refunded"));
        assert_eq!(topic0(&list[3].body), sym("request_expired"));
        assert_eq!(topic0(&list[4].body), sym("initialized"));
    }
}
