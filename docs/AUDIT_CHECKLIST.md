# Pre-audit checklist (Soroban / Stellar style)

Self-assessment with evidence test names. All items ticked or explicitly
risk-accepted for v1.

## Authorization

- [x] Every privileged entrypoint authenticates an explicit `caller`
      (`caller.require_auth()` + role comparison) — no confused-deputy.
      Evidence: `test::auth_matrix` (12 combos), `test::admin_authz_denies_strangers`.
- [x] `deposit` authorizes the funder; any authenticated depositor accepted
      (allowlisting is API-layer). Evidence: `initialize_requires_admin_auth`
      pattern + fuzz (auth always mocked, roles still enforced).
- [x] `expire` is permissionless by design (can only pay the depositor).
      Evidence: `expire_by_third_party_refunds_depositor` (zero mocks).
- [x] Unauthenticated calls trap at the host before contract code runs.
      Evidence: `initialize_requires_admin_auth`.
- [x] Two-step admin rotation (stage + accept). Evidence: `admin_rotation_two_step`.
- [x] Signer rotation is immediate; old signer denied typed.
      Evidence: `signer_rotation_invalidates_old_signer`.

## Initialization

- [x] One-time `initialize`; second call fails `AlreadyExists` (double-init
      cannot seize admin). Evidence: `initialize_twice_fails`.
- [x] Uninitialized entrypoints fail `NotFound` (no default roles).
      Evidence: role helpers `ok_or(NotFound)`; covered via rotation tests.

## Arithmetic

- [x] Amounts are `i128` stroops with `MAX_AMOUNT` (50M XLM) guard.
      Evidence: `amounts_*`, `overflow_deposit_i128_max_rejected`.
- [x] All counter updates `checked_*` → `Overflow`; no wrapping.
      Evidence: `amounts_overflow_guarded`, `overflow_release_underflow_guarded`,
      `overflow_deposit_counter_saturated`.
- [x] Failed arithmetic rolls back atomically (no partial writes).
      Evidence: `amounts_overflow_guarded` (request absent after `Overflow`).

## TTL / storage

- [x] Every write extends TTL to 30d ≥ max deadline. Evidence: storage unit
      tests + `ttl_extended_on_every_write`.
- [x] Views never bump TTL. Evidence: `get_request`/`list_requests` use only
      `storage::get_*` (code inspection; no `extend` call outside `set_*`).
- [x] Pagination bounded (`limit ≤ 50`, window slicing). Evidence:
      `list_requests_rejects_over_cap`, `list_requests_pages_seeded_registry`.
- [x] Dormant-entry eviction analyzed; keeper rule + upgrade recovery
      documented. Risk-accepted for v1 — see `docs/STORAGE.md` §Eviction.

## Events

- [x] Exactly one typed event per mutation; none on failure. Evidence:
      `events_suite_*` (topic + full payload decode), `test::reentrancy_*`,
      `events_suite_failed_calls_emit_nothing`.
- [x] `expire` attributes the refunded depositor (documented, since the call
      itself carries no identity). Evidence: `events_suite_expire_payload`.

## Pause / upgrade

- [x] Pause blocks deposit/release/refund, never `expire`. Evidence:
      `pause_blocks_mutations_but_not_expire`.
- [x] Pause/unpause emit events. Evidence: `pause_*`, `unpause_resumes_and_emits`.
- [x] `migrate` is admin-only; state-compat checklist in `docs/UPGRADE.md`.
      Evidence: `migrate_requires_admin`, `migrate_by_admin_passes_auth_gate`.
- [x] `version()` pins the schema. Evidence: `version_returns_schema_version`.

## Denial of service

- [x] No unbounded loops (only capped `list_requests` window). See `docs/GAS.md`.
- [x] `AllIds` is append-only; terminal requests stay listed (no deletion
      paths to abuse). Filtered scans are client-paged (documented).
- [x] Fuzz: 10k adversarial deposits → typed errors only, zero host traps.
      Evidence: `tests/fuzz.rs`.

## Front-running

- [x] `expire` permissionless by design; frontrunning an expiry only pays the
      depositor sooner (no MEV — same recipient, same amount).
- [x] `release`/`refund` race: first valid settler wins; second fails
      `InvalidState` with balances unchanged. Evidence: `edge_double_ops_move_no_funds`.

## Token trust

- [x] Exactly one token interface: the SAC id stored at `initialize`.
      No arbitrary-token callbacks; the host SAC cannot reenter.
      Evidence: `docs/XLM_DESIGN.md` + `reentrancy_failed_transfer_rolls_back`.
- [x] Status is written BEFORE the external transfer
      (checks-effects-interactions) in all four money-moving entrypoints
      (code inspection: `set_request` precedes `token::push`/`pull`).

## Double-settlement

- [x] Each request settles at most once; terminal states never transition.
      Refund-from-`Expired` deliberately rejected (would double-pay since
      `expire` already transfers). Evidence: `edge_double_ops_move_no_funds`,
      `expire_after_release_fails`, `refund_guards_after_release`.
- [x] Failed transfers roll back state + events atomically. Evidence:
      `reentrancy_failed_transfer_rolls_back`.

## Residual risks (accepted)

1. Dormant-entry eviction past 30d (keeper rule mitigates; upgrade recovery
   documented). See `docs/STORAGE.md`.
2. `AllIds` linear growth raises `deposit` cost over years (O(n) in-memory
   dedupe scan, no extra ledger reads); migration path noted in `docs/GAS.md`.
3. Ledger-timestamp granularity (~5s) on deadline edges; API 300s grace
   absorbs it. See `docs/STORAGE.md`.
