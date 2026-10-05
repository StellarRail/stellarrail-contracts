# Changelog

All notable changes to the escrow contract. Versioning policy below.

## [Unreleased]

## [v1.0.0-rc1] — 2026-10-04 (this build plan)

First production-release candidate: full FR-4 escrow surface, audited
(self-assessment), gas-measured, live on testnet.

### Added — Phase A (scaffold)

- Cargo workspace + `escrow` crate (soroban-sdk 28.0.0, rust 1.97.1,
  `wasm32v1-none`), Makefile, toolchain pins (`docs/TOOLCHAIN.md`).
- Clippy `-D warnings` + fmt + pre-commit hook; module layout
  (`lib/storage/types/events/errors/admin/validation/token`) + `ARCHITECTURE.md`.
- Canonical types (`EscrowRequest`, `EscrowStatus`, `SCHEMA_VERSION = 1`),
  typed `EscrowError` codes 1–11 (`docs/ERRORS.md`).
- TTL-extending storage helpers + `docs/STORAGE.md`; five lifecycle events +
  `docs/EVENTS.md`; sandbox/invoke scripts + `docs/LOCAL_DEV.md`; SAC-XLM
  design (`docs/XLM_DESIGN.md`, `src/token.rs`).

### Added — Phase B (core logic)

- `initialize(admin, signer, token)` (one-time, `AlreadyExists` on repeat).
- `deposit` (validation, SAC pull, `Locked` persist, `FundsLocked`),
  `release`/`refund` (admin-or-signer via explicit `caller`, checks-before-
  interactions, settlement push), permissionless `expire` (past deadline,
  works paused).
- Views: `get_request`, capped `list_requests` pagination,
  O(1) `get_stats` counters maintained atomically with every write.
- Deadline bounds (60s–30d), stroops `MAX_AMOUNT` guard, checked arithmetic,
  pause switch, two-step admin rotation + immediate signer rotation,
  `version()` + `migrate()` + `docs/UPGRADE.md`, `docs/GAS.md`.

### Added — Phase C (hardening)

- 12-combo auth matrix (`docs/AUTH_MATRIX.md`); 10k-iteration deterministic
  fuzz (`tests/fuzz.rs`); double-op balance invariance; overflow/underflow
  guards; pedantic-clean clippy; full event payload assertions; per-write
  TTL guarantees; `docs/AUDIT_CHECKLIST.md`; `SECURITY.md` (threat model,
  invariants, disclosure).

### Added — Phase D (views/indexing/sdk)

- `docs/INDEXER.md` (getEvents filters, TS sketch, idempotency/reorg rules);
  `clients/js` invocation snippets + error mapping; per-network CLI scripts;
  `docs/DEPLOY_TESTNET.md` + live deployment
  (`CDAV32AHVV6Q7FFNPUFA76AARTGWBE2QFIU3WAPDV64QIBRTGUFKLU2K`);
  `docs/DEPLOY_MAINNET.md` guardrails; README.

### Added — Phase E (testnet/integration/perf)

- Real-WASM sandbox suite; automated `smoke-testnet.sh` + live 1-XLM
  evidence; third-party `expire-drill.sh` + evidence; live v1→v1 upgrade
  drill with state preservation (`deployments/upgrade-drill.md`); measured
  testnet fee table; 100-deposit soak; panic/unwrap policy; docs polish +
  ADR-003; JS client tests; reproducible-build registry; this changelog;
  `SECURITY.md` final; interface freeze + ADRs.

## Versioning policy

- **Interface change** (entrypoint added/removed/signature changed, error
  code renumbered, event topic/payload changed, storage key/field changed
  incompatibly) = **major** bump + `SCHEMA_VERSION` bump + migration notes.
- **Additive view or compatible extension** (new read-only view, new event,
  new error code appended) = **minor** bump.
- **Bug fix with identical interface** = **patch** bump.
- `version()` must equal the release tag: the release workflow refuses to
  cut `contracts-vX.Y.Z` unless on-chain `version()` matches
  `SCHEMA_VERSION` for that tag.
- `CHANGELOG.md` entry required for every shipped change; release tags look
  like `contracts-v1.0.0-rc1`.
