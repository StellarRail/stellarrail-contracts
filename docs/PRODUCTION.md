# Production checklist (pre-release gate)

Tick every box before mainnet pilot. Evidence links required per item.

## Build & test

- [x] Pinned toolchain reproduces (`rust 1.97.1`, `soroban-sdk 28.0.0`,
      `stellar-cli 27.1.0`) — `docs/TOOLCHAIN.md`.
- [x] `cargo fmt --check`, `clippy --all-targets -- -D warnings`
      (+ pedantic clean), `cargo test --all-targets` green.
- [x] WASM 20 468 bytes (< 50 KiB), `sha256 de7f457c…fbc1a` reproducible
      across consecutive builds — `deployments/builds.json`.
- [x] Fuzz 10k inputs: typed errors only, zero host traps (`tests/fuzz.rs`).
- [x] Soak 100 deposits + mixed settles green in ~2s (`tests/soak.rs`).

## Audit & freeze

- [x] `docs/AUDIT_CHECKLIST.md` self-assessment: all ticked or risk-accepted.
- [x] Threat model + invariants (`SECURITY.md`), each invariant mapped to a test.
- [x] Interface frozen (README freeze note) + ADRs 001–003 in `docs/adr/`.
- [ ] **Independent external audit: PENDING — mainnet pilot requires it.**

## Testnet proof

- [x] Live deployment `CDAV32AH…LU2K` (`deployments/testnet.json`).
- [x] 1-XLM deposit→release smoke (`deployments/smoke-*.json`, automated in
      `scripts/smoke-testnet.sh`).
- [x] Third-party expire drill (`deployments/expire-drill-*.json`,
      `scripts/expire-drill.sh`).
- [x] v1→v1 upgrade drill, state preserved (`deployments/upgrade-drill.md`).
- [x] Measured fee table (`docs/GAS.md`): deposit ~0.19 XLM, settlement
      ~0.002 XLM legs.

## Mainnet pilot (pending — see `docs/DEPLOY_MAINNET.md`)

- [ ] Multisig admin + HSM signer provisioned; key-holder list in `SECURITY.md`.
- [ ] `CONFIRM_MAINNET` ceremony rehearsed on testnet.
- [ ] Event poller live (`docs/INDEXER.md`) before first deposit.
- [ ] Keeper cron live (10-min expire sweep, `docs/DISASTER_RECOVERY.md`).
- [ ] Pilot limits enforced: ≤ 10 XLM/escrow, ≤ 100 XLM aggregate, 7 clean days.
- [ ] Rollback rehearsed: pause drill + `migrate` drill on testnet.

## Rollback plan

Pause (`set_paused`) → diagnose → fix → `migrate`, or CLI-direct
settlement without the API. Details: `docs/DISASTER_RECOVERY.md`.
