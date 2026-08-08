# BUILD PLAN: stellarrail-contracts — 70 Issues to Production Ready

> **Repo:** `stellarrail-contracts` (Rust, Soroban SDK, cargo test + sandbox/testnet integration)
> **Source of truth:** `../PRD.md` FR-4 + `../Architecture.md` §2.3/§5.2
> **Goal:** Production-ready Soroban Escrow Contract: `deposit`, `release`, `refund`, `expire` (+ admin, events, upgradeability) — audited, gas-efficient, tested, deployable to Testnet → Mainnet.

**Contract interface (v1 locked):**
```rust
deposit(amount: i128, request_id: BytesN<32>, depositor: Address, deadline: u64)
release(request_id: BytesN<32>)
refund(request_id: BytesN<32>)
expire(request_id: BytesN<32>)
get_request(request_id: BytesN<32>) -> EscrowRequest
list_requests(status_filter: Option<EscrowStatus>, offset: u32, limit: u32) -> Vec<EscrowRequest>
```
Events: `FundsLocked`, `PaymentReleased`, `PaymentRefunded`, `RequestExpired`. Access: `release/refund` = admin/signer via `require_auth()`.

---

## AGENT EXECUTION PROMPT (MUST FOLLOW)

You are the builder agent for `stellarrail-contracts`. Execute issues **strictly sequentially from ISSUE-001 to ISSUE-070**.

Rules:
1. **One issue at a time.** Fully implement before next.
2. **Verify each issue** with listed commands (`cargo fmt --check`, `cargo clippy`, `cargo test`, `soroban` CLI or `stellar` CLI where given). Fix failures before committing.
3. **Commit after EACH issue:**
   ```bash
   git add -A
   git commit -m "<type>(contracts): <short description> [ISSUE-0XX]"
   ```
   Use exact message per issue. `git init` once at ISSUE-001 if needed.
4. **Do not proceed** dirty or red.
5. **Production bar:** No `unwrap()` in contract code (only tests), checked arithmetic, TTL extended on every write, events on every mutation, 100% branch coverage on `deposit/release/refund/expire`, documented storage schema, reproducible builds.
6. At ISSUE-068–070 set up **GitHub Actions workflows**. After ISSUE-070 CI green on push/PR.
7. If satisfied already, verify + commit (or `--allow-empty` verified).

Types: `feat`, `fix`, `chore`, `test`, `docs`, `ci`, `security`, `perf`, `refactor`.

Start at ISSUE-001.

---

## PHASE A — SCAFFOLD & TOOLCHAIN (001–010)

### ISSUE-001: Initialize Cargo workspace + Soroban crate
**Goal:** Compilable contract crate.
**Tasks:**
- `cargo init --lib` (or `soroban init`), workspace `Cargo.toml`, crate `escrow` with `soroban-sdk`, `crate-type = ["cdylib"]`, rust-toolchain `stable` (pin version in `rust-toolchain.toml`), `.editorconfig`, `README.md` stub.
- `src/lib.rs` hello returning 0 + `cargo test` green.
**Acceptance:** `cargo build --target wasm32-unknown-unknown` succeeds (or `soroban build`).
**Verify:** `cargo test && cargo fmt --check`
**Commit:** `chore(contracts): init cargo soroban crate [ISSUE-001]`

### ISSUE-002: Toolchain pins (rust, soroban-cli, node for scripts)
**Goal:** Reproducible builds.
**Tasks:**
- Pin `soroban-sdk` version (e.g., 21.x — record exact), `stellar-cli` install docs, `.nvmrc` for JS scripts, `Makefile` targets: `build`, `test`, `fmt`, `lint`, `deploy-testnet`.
- `docs/TOOLCHAIN.md` with versions + `rustc --version` output.
**Acceptance:** Fresh machine follows doc to green `make test`.
**Verify:** `make test`
**Commit:** `chore(contracts): toolchain pins makefile [ISSUE-002]`

### ISSUE-003: Clippy + fmt + commit hooks
**Goal:** Zero-warning codebase.
**Tasks:**
- `#![deny(warnings)]`? (allow in tests), `cargo clippy -- -D warnings`, `cargo fmt`, pre-commit hook via `husky`/`pre-commit` or `cargo-husky`.
- CI-ready `make lint`.
**Acceptance:** `make lint` clean.
**Verify:** `cargo clippy --all-targets -- -D warnings && cargo fmt --check`
**Commit:** `chore(contracts): clippy fmt hooks [ISSUE-003]`

### ISSUE-004: Project layout (lib, storage, types, events, errors, admin)
**Goal:** Separation per Arch.
**Tasks:**
- `src/{lib.rs, storage.rs, types.rs, events.rs, errors.rs, admin.rs, validation.rs}` + `tests/` dir. Document module responsibilities in `ARCHITECTURE.md` (contract-level).
**Acceptance:** `cargo test` still green, docs match tree.
**Verify:** `ls -R src tests`
**Commit:** `chore(contracts): module layout docs [ISSUE-004]`

### ISSUE-005: Types (EscrowRequest, EscrowStatus enum)
**Goal:** Canonical schema.
**Tasks:**
- `types.rs`: `EscrowStatus {Created, Locked, Released, Refunded, Expired, Failed}`, `EscrowRequest {request_id, depositor, destination: Option<Address>, amount: i128, deadline, status, created_at, updated_at, release_hash?: BytesN<32>}` — `contracttype` derived, versioned (`SCHEMA_VERSION: u32 = 1`).
**Acceptance:** Types compile + doc comments with field semantics.
**Verify:** `cargo test`
**Commit:** `feat(contracts): escrow types schema [ISSUE-005]`

### ISSUE-006: Errors (typed ContractError enum)
**Goal:** Machine-readable failures for API mapping.
**Tasks:**
- `errors.rs`: `#[contracterror] EscrowError {AlreadyExists=1, NotFound=2, Unauthorized=3, InvalidAmount=4, InvalidDeadline=5, InvalidState=6, Expired=7, NotExpired=8, Overflow=9, Paused=10}`. Map to API codes in `docs/ERRORS.md` table.
**Acceptance:** Error codes doc complete.
**Verify:** `cargo test`
**Commit:** `feat(contracts): typed errors docs [ISSUE-006]`

### ISSUE-007: Storage schema + TTL strategy + keys
**Goal:** No data loss / no bloat.
**Tasks:**
- `storage.rs`: keys `Request(request_id)`, `Admin`, `Signer`, `Paused`, `Counter`; helpers `get_request/set_request` that **extend TTL** (persistent `extend_ttl(threshold, extend_to)` e.g., 30d) on every write; `docs/STORAGE.md` diagram.
**Acceptance:** TTL extension asserted in unit test (mock env ledger).
**Verify:** `cargo test storage`
**Commit:** `feat(contracts): storage ttl schema [ISSUE-007]`

### ISSUE-008: Events (FundsLocked, PaymentReleased, PaymentRefunded, RequestExpired)
**Goal:** FR-5 indexing for API workers.
**Tasks:**
- `events.rs`: `#[contractevent]` structs with `request_id`, `amount`, `actor`, `ledger_time`; emit on every mutation; `docs/EVENTS.md` (topics, fields, example JSON for indexer).
**Acceptance:** Test asserts event published (soroban `env.events()` mock check).
**Verify:** `cargo test events`
**Commit:** `feat(contracts): escrow events [ISSUE-008]`

### ISSUE-009: Local sandbox + helper scripts (deploy, invoke locally)
**Goal:** Fast inner loop.
**Tasks:**
- `scripts/sandbox.sh` (spin `stellar node`/sandbox or `soroban test` env), `scripts/invoke.sh` wrappers for 4 entrypoints, `Makefile deploy-local`.
- Document in `docs/LOCAL_DEV.md`.
**Acceptance:** `make deploy-local` + invoke deposit works locally.
**Verify:** `make deploy-local || cargo test`
**Commit:** `chore(contracts): sandbox scripts [ISSUE-009]`

### ISSUE-010: Example XLM (native) handling decision + docs
**Goal:** XLM-only v1 clarity (PRD Non-Goal: no custom assets yet, but design for SAC).
**Tasks:**
- Decide: native XLM via `transfer` from depositor (requires auth) vs SAC token client. v1: use Stellar Asset Contract client for `XLM` (document contract ID per network). Record in `docs/XLM_DESIGN.md` with sequence diagram.
**Acceptance:** Decision doc + stub `token_client` wiring compiles.
**Verify:** `cargo test`
**Commit:** `docs(contracts): xlm handling design [ISSUE-010]`

---

## PHASE B — CORE LOGIC (011–028)

### ISSUE-011: `initialize(admin: Address, signer: Address)`
**Goal:** One-time setup.
**Tasks:**
- Sets Admin + authorized Signer, `require_auth()` on admin, guard double-init (`AlreadyInitialized`), emits `Initialized` event, sets `Paused=false`.
**Acceptance:** Second init panics `AlreadyInitialized`.
**Verify:** `cargo test initialize`
**Commit:** `feat(contracts): initialize [ISSUE-011]`

### ISSUE-012: `deposit(amount, request_id, depositor, destination?, deadline)` — validation
**Goal:** FR-4.1 input gates.
**Tasks:**
- Checks: `amount > 0` else `InvalidAmount`; `request_id` unused else `AlreadyExists`; `deadline > now` else `InvalidDeadline`; `depositor.require_auth()`; destination optional-valid.
**Acceptance:** Zero amount / past deadline tests panic with correct codes.
**Verify:** `cargo test deposit_validation`
**Commit:** `feat(contracts): deposit validation [ISSUE-012]`

### ISSUE-013: `deposit` — token transfer + state + event
**Goal:** FR-4.1 lock funds.
**Tasks:**
- Transfer XLM (SAC `transfer_from`/mint mock in tests) depositor→contract, persist `Locked` request, extend TTL, emit `FundsLocked`. Return request.
**Acceptance:** Balance assertion in test (mock token).
**Verify:** `cargo test deposit`
**Commit:** `feat(contracts): deposit transfer event [ISSUE-013]`

### ISSUE-014: `release(request_id)` — auth + state guards
**Goal:** FR-4.2 gates.
**Tasks:**
- Require caller is Admin OR Signer (`require_auth`, else `Unauthorized`), request must be `Locked`, `now <= deadline` else `Expired` (direct to expire path hint in error).
**Acceptance:** Non-admin release panics `Unauthorized`; double-release panics `InvalidState`.
**Verify:** `cargo test release_guards`
**Commit:** `feat(contracts): release guards [ISSUE-014]`

### ISSUE-015: `release` — transfer + event + status
**Goal:** FR-4.2 settle.
**Tasks:**
- Transfer amount contract→destination, set `Released`, `updated_at=now`, extend TTL, emit `PaymentReleased`.
**Acceptance:** Destination balance += amount in mock.
**Verify:** `cargo test release`
**Commit:** `feat(contracts): release transfer [ISSUE-015]`

### ISSUE-016: `refund(request_id)` — auth + guards
**Goal:** FR-4.3 gates.
**Tasks:**
- Same auth as release; allowed from `Locked` (rejection path) and `Expired`; not from `Released/Refunded` → `InvalidState`.
**Acceptance:** Refund after release panics.
**Verify:** `cargo test refund_guards`
**Commit:** `feat(contracts): refund guards [ISSUE-016]`

### ISSUE-017: `refund` — transfer back + event
**Goal:** FR-4.3 return to depositor.
**Tasks:**
- Transfer contract→depositor, set `Refunded`, emit `PaymentRefunded`.
**Acceptance:** Depositor made whole in mock.
**Verify:** `cargo test refund`
**Commit:** `feat(contracts): refund transfer [ISSUE-017]`

### ISSUE-018: `expire(request_id)` — permissionless, deadline enforced
**Goal:** FR-4.4 gas-efficient cleanup.
**Tasks:**
- Callable by **anyone** (no auth), requires `now > deadline` else `NotExpired`, requires `Locked` else `InvalidState`; performs same transfer as refund, sets `Expired`, emits `RequestExpired`.
**Acceptance:** Early expire panics `NotExpired`; anyone (random addr) can expire past-due.
**Verify:** `cargo test expire`
**Commit:** `feat(contracts): expire permissionless [ISSUE-018]`

### ISSUE-019: `get_request(request_id)` view
**Goal:** API + UI reads.
**Tasks:**
- Returns `EscrowRequest` or `NotFound`. No auth, no TTL bump (read-only).
**Acceptance:** Unknown ID panics `NotFound`.
**Verify:** `cargo test get_request`
**Commit:** `feat(contracts): get_request view [ISSUE-019]`

### ISSUE-020: `list_requests(status?, offset, limit)` pagination
**Goal:** Admin/recon scans.
**Tasks:**
- Iterate range or index list (maintain `AllIds` vec or ledger `persistent` map scan — document choice + gas note, cap `limit<=50`), filter by status, deterministic order.
**Acceptance:** 25 seeded → page 1/2 correct.
**Verify:** `cargo test list_requests`
**Commit:** `feat(contracts): list pagination [ISSUE-020]`

### ISSUE-021: Deadline handling (min/max bounds, ledger-time source)
**Goal:** Safe expiries.
**Tasks:**
- Enforce `MIN_DEADLINE_SECS=1h`, `MAX=30d` from `created_at`; use `env.ledger().timestamp()`; document clock-skew note for API (5-min grace recommendation).
**Acceptance:** 10-year deadline rejected.
**Verify:** `cargo test deadline_bounds`
**Commit:** `feat(contracts): deadline bounds [ISSUE-021]`

### ISSUE-022: Amount precision (stroops i128, max supply guard)
**Goal:** XLM safety.
**Tasks:**
- Amounts in stroops (`i128`), `MAX_AMOUNT = 50_000_000_000 * 10^7` guard → `InvalidAmount`; checked arithmetic everywhere (`checked_add` or panic `Overflow`).
**Acceptance:** Overflow test panics `Overflow`.
**Verify:** `cargo test amounts`
**Commit:** `feat(contracts): stroops precision [ISSUE-022]`

### ISSUE-023: Reentrancy/state-consistency (checks-effects-interactions)
**Goal:** Arch §5.2.
**Tasks:**
- Set status BEFORE external token transfer (or document Soroban host non-reentrancy + still order writes first); add test simulating callback attempt (mock token that calls back → still single transfer).
**Acceptance:** Test + comment in code.
**Verify:** `cargo test reentrancy`
**Commit:** `security(contracts): reentrancy guard [ISSUE-023]`

### ISSUE-024: Pause / emergency stop (admin only)
**Goal:** Incident response.
**Tasks:**
- `set_paused(bool)` admin-only; when paused, `deposit/release/refund` panic `Paused` (expire still allowed — funds must remain rescuable); event `Paused/Unpaused`.
**Acceptance:** Paused deposit panics; expire still works.
**Verify:** `cargo test paused`
**Commit:** `feat(contracts): pause switch [ISSUE-024]`

### ISSUE-025: Admin transfer + signer rotation
**Goal:** Quarterly rotation NFR.
**Tasks:**
- `transfer_admin(new_admin)` (old admin auth, new admin must auth on `accept_admin` two-step), `set_signer(new_signer)` admin-only + event. Old signer immediately invalid.
**Acceptance:** Old signer release after rotation → `Unauthorized`.
**Verify:** `cargo test admin_rotation`
**Commit:** `feat(contracts): admin signer rotation [ISSUE-025]`

### ISSUE-026: Upgradeability (proxy/migration entrypoint)
**Goal:** Arch §5.2 upgradability without losing state.
**Tasks:**
- `version() -> u32`, `migrate()` admin-only stub calling `env.deployer().update_current_contract_wasm(hash)` + auth; `docs/UPGRADE.md` (testnet drill steps, state-compat checklist).
**Acceptance:** `version()` returns SCHEMA_VERSION; migrate callable only by admin in test.
**Verify:** `cargo test version`
**Commit:** `feat(contracts): upgrade migrate [ISSUE-026]`

### ISSUE-027: Gas notes + TTL auto-extend audit
**Goal:** Cost control.
**Tasks:**
- Document per-entrypoint footprint estimate table (read/write counts), assert no unbounded loops (limit caps), `docs/GAS.md`.
**Acceptance:** `list_requests` with limit 1000 rejected (cap).
**Verify:** `cargo test gas_caps`
**Commit:** `docs(contracts): gas ttl notes [ISSUE-027]`

### ISSUE-028: Core logic integration test (deposit→release, deposit→refund, deposit→expire)
**Goal:** Happy paths locked.
**Tasks:**
- `tests/happy_path.rs` (soroban `Env` + mock token + time-travel via `env.ledger().set_timestamp`): three lifecycles green.
**Acceptance:** All pass.
**Verify:** `cargo test --test happy_path`
**Commit:** `test(contracts): happy paths [ISSUE-028]`

---

## PHASE C — ACCESS CONTROL & HARDENING (029–038)

### ISSUE-029: Auth matrix test (admin/signer/random × 4 entrypoints)
**Goal:** Arch §5.2 access control proof.
**Tasks:**
- Exhaustive table test asserting allowed/denied per caller; document matrix in `docs/AUTH_MATRIX.md`.
**Acceptance:** 12 combos asserted.
**Verify:** `cargo test auth_matrix`
**Commit:** `test(contracts): auth matrix [ISSUE-029]`

### ISSUE-030: Fuzz/property tests (amounts, ids, deadlines)
**Goal:** Panic-freedom on adversarial inputs.
**Tasks:**
- `proptest` or manual fuzz loop: random amounts (neg, 0, max+1), random ids, past/future deadlines → only expected errors, never trap without code.
**Acceptance:** 10k iterations green.
**Verify:** `cargo test fuzz`
**Commit:** `test(contracts): fuzz inputs [ISSUE-030]`

### ISSUE-031: Edge cases (double deposit id, release twice, refund twice, expire twice)
**Goal:** Double-spend impossible.
**Tasks:**
- Each double-op test asserts second fails + balances unchanged.
**Acceptance:** Green.
**Verify:** `cargo test edge_double`
**Commit:** `test(contracts): double-op edges [ISSUE-031]`

### ISSUE-032: Overflow/underflow + i128 boundary tests
**Goal:** Safe arithmetic.
**Tasks:**
- `i128::MAX` deposit → `InvalidAmount/Overflow`; release math checked; clippy pedantic clean.
**Acceptance:** Green.
**Verify:** `cargo test overflow`
**Commit:** `test(contracts): overflow bounds [ISSUE-032]`

### ISSUE-033: Paused + upgrade auth tests
**Goal:** Lock admin surface.
**Tasks:**
- Non-admin `set_paused/migrate/set_signer` → `Unauthorized`.
**Acceptance:** Green.
**Verify:** `cargo test admin_auth`
**Commit:** `test(contracts): admin authz [ISSUE-033]`

### ISSUE-034: Event assertion suite (every mutation emits exactly 1 typed event)
**Goal:** Indexer reliability.
**Tasks:**
- For each of deposit/release/refund/expire assert topic + payload; test no-event on failed paths.
**Acceptance:** Green.
**Verify:** `cargo test events_suite`
**Commit:** `test(contracts): event assertions [ISSUE-034]`

### ISSUE-035: Storage TTL test (extend on write, expiry simulation)
**Goal:** No silent eviction.
**Tasks:**
- Assert TTL extended after each write; simulate expired ledger entry handling (doc recovery note).
**Acceptance:** Green + doc.
**Verify:** `cargo test ttl`
**Commit:** `test(contracts): ttl guarantees [ISSUE-035]`

### ISSUE-036: Audit checklist pass (Soroban + Stellar quest style)
**Goal:** Pre-audit gate.
**Tasks:**
- `docs/AUDIT_CHECKLIST.md`: auth, init, arithmetic, TTL, events, pause, upgrade, DoS (unbounded), front-running (expire permissionless note), token trust (SAC only). Self-score each + evidence test name.
**Acceptance:** All ticked or risk-accepted.
**Verify:** doc review
**Commit:** `security(contracts): audit checklist [ISSUE-036]`

### ISSUE-037: Gas optimization pass (clone/copies, vec caps, short-circuit)
**Goal:** Cheap cleanup (expire callable by anyone).
**Tasks:**
- Minimize clones, early-return errors before reads, cap loops; `cargo bloat`-ish size note + before/after WASM size in `docs/GAS.md`.
**Acceptance:** WASM size recorded, no clippy perf warnings.
**Verify:** `cargo clippy -- -W clippy::perf && ls -lh target/wasm32*/release/*.wasm`
**Commit:** `perf(contracts): gas optimization [ISSUE-037]`

### ISSUE-038: External review prep (README threat model + invariants)
**Goal:** Auditable.
**Tasks:**
- `SECURITY.md`: threat model (malicious operator, rogue approver, chain reorg note), invariants (locked supply == sum Locked; released/refunded terminal; only admin/signer move funds; expired always refundable), contact + disclosure.
**Acceptance:** Invariants each map to a test.
**Verify:** doc review
**Commit:** `docs(contracts): threat model invariants [ISSUE-038]`

---

## PHASE D — VIEWS, INDEXING, SDK DOCS (039–046)

### ISSUE-039: `get_stats()` view (counts by status, total locked)
**Goal:** Dashboard + recon support.
**Tasks:**
- Returns `{locked_count, locked_total, released_count, lifetime_volume}` (bounded computation — maintain counters on write, not scan).
**Acceptance:** Counters correct after mixed ops test.
**Verify:** `cargo test stats`
**Commit:** `feat(contracts): get_stats view [ISSUE-039]`

### ISSUE-040: Counter maintenance (increment on deposit, decrement on terminal)
**Goal:** O(1) stats.
**Tasks:**
- `Stats {…}` in storage updated atomically with request writes; overflow-checked.
**Acceptance:** Fuzz lifecycles keep counters consistent.
**Verify:** `cargo test counters`
**Commit:** `feat(contracts): stats counters [ISSUE-040]`

### ISSUE-041: Indexer guide for API team (topics → DB mapping)
**Goal:** FR-5 unblock API workers.
**Tasks:**
- `docs/INDEXER.md`: event topic strings, RPC `getEvents` filters (contractId + topics), sample `stellar` CLI + TS snippet, idempotency note (tx hash dedupe), reorg handling (wait N ledgers).
**Acceptance:** API dev can implement poller from doc alone (review).
**Verify:** doc review
**Commit:** `docs(contracts): indexer guide [ISSUE-041]`

### ISSUE-042: TypeScript bindings / invocation snippets
**Goal:** Chain service velocity.
**Tasks:**
- `clients/js/{invoke.ts, types.ts}`: `deposit/release/refund/expire/get_request` wrappers with `soroban-client`/`stellar-sdk`, arg builders (BytesN<32> from uuid), error-code mapping table.
**Acceptance:** `npx tsc --noEmit` on clients/ green.
**Verify:** `npx tsc --noEmit -p clients/js || echo documented`
**Commit:** `feat(contracts): js invocation snippets [ISSUE-042]`

### ISSUE-043: CLI scripts (deploy + invoke + query per network)
**Goal:** Ops without API (Arch §7 manual fallback).
**Tasks:**
- `scripts/{deploy-testnet.sh, deploy-mainnet.sh (guarded), invoke-deposit.sh, invoke-release.sh, invoke-refund.sh, invoke-expire.sh, query.sh}` with `set -euo pipefail`, env checks, confirm prompts for mainnet.
**Acceptance:** Shellcheck clean.
**Verify:** `shellcheck scripts/*.sh || bash -n scripts/*.sh`
**Commit:** `feat(contracts): cli scripts [ISSUE-043]`

### ISSUE-044: Testnet deployment runbook
**Goal:** Alpha ready (PRD §7).
**Tasks:**
- `docs/DEPLOY_TESTNET.md`: Friendbot funding, `stellar contract deploy`, `initialize` with test admin/signer, SAC XLM wiring, 1-XLM smoke (deposit→release, deposit→expire), explorer links.
**Acceptance:** Followed once end-to-end (record contract ID + tx hashes in `deployments/testnet.json`).
**Verify:** doc + `cat deployments/testnet.json`
**Commit:** `docs(contracts): testnet runbook [ISSUE-044]`

### ISSUE-045: Mainnet deployment guardrails + checklist
**Goal:** RC1 safety.
**Tasks:**
- `docs/DEPLOY_MAINNET.md`: small-value pilot steps, `CONFIRM_MAINNET` env gate in scripts, multisig admin recommendation, rollback (migrate) plan, `deployments/mainnet.json` template (empty until pilot).
- Scripts refuse mainnet without explicit flag + 10s countdown.
**Acceptance:** Accidental mainnet deploy impossible in test (assert script exits without flag).
**Verify:** `./scripts/deploy-mainnet.sh || true` exits non-zero safely
**Commit:** `feat(contracts): mainnet guardrails [ISSUE-045]`

### ISSUE-046: Contract README (usage, interface, errors, networks)
**Goal:** Adoptability.
**Tasks:**
- Root `README.md`: what/why, interface table, error table, quickstart (build→test→deploy-local→testnet), network contract IDs, audit status badge placeholder.
**Acceptance:** New dev runs quickstart unaided.
**Verify:** peer-run once
**Commit:** `docs(contracts): readme [ISSUE-046]`

---

## PHASE E — TESTNET, INTEGRATION, PERF (047–060)

### ISSUE-047: Sandbox integration suite (real WASM in local env)
**Goal:** Beyond unit mocks.
**Tasks:**
- `tests/integration_sandbox.rs`: deploy compiled WASM to `Env` with real ledger footprint, run full lifecycles + pause + rotation.
**Acceptance:** Green.
**Verify:** `cargo test --test integration_sandbox`
**Commit:** `test(contracts): sandbox integration [ISSUE-047]`

### ISSUE-048: Testnet smoke (1 XLM deposit→release on live testnet)
**Goal:** Live proof.
**Tasks:**
- `scripts/smoke-testnet.sh`: creates 2 temp accounts, funds via Friendbot, deposits 1 XLM, releases, asserts balances; records hashes to `deployments/smoke-*.json`. Skippable in CI without secrets (`SKIP_LIVE=1`).
**Acceptance:** Hashes recorded + explorer links.
**Verify:** `SKIP_LIVE=1 ./scripts/smoke-testnet.sh || true`
**Commit:** `test(contracts): testnet smoke [ISSUE-048]`

### ISSUE-049: Expire live drill (deadline 60s → permissionless expire by third party)
**Goal:** FR-4.4 live proof.
**Tasks:**
- Script deposits with 60s deadline, waits, expires from different account, asserts refund. Document timing.
**Acceptance:** Evidence JSON saved.
**Verify:** manual / `SKIP_LIVE=1` stub green
**Commit:** `test(contracts): expire drill [ISSUE-049]`

### ISSUE-050: Upgrade drill on testnet (v1 → v1 mock, state preserved)
**Goal:** Arch upgradability proof.
**Tasks:**
- Deploy v1, create 2 locked requests, `migrate` to rebuilt WASM (bump version), assert requests + stats intact.
**Acceptance:** Evidence log in `deployments/upgrade-drill.md`.
**Verify:** doc + local simulate green
**Commit:** `test(contracts): upgrade drill [ISSUE-050]`

### ISSUE-051: Gas/footprint report (per-entrypoint, WASM size, ledger entries)
**Goal:** Cost transparency for API fee estimation.
**Tasks:**
- `make report-gas` (soroban footprint snapshot or manual table), publish to `docs/GAS.md` with testnet numbers.
**Acceptance:** Table complete for 6 entrypoints.
**Verify:** `make report-gas || cargo test`
**Commit:** `perf(contracts): gas report [ISSUE-051]`

### ISSUE-052: Soak test (100 sequential deposits + mixed settles locally)
**Goal:** No storage blowup.
**Tasks:**
- Loop 100 deposits, settle 50, refund 25, expire 25; assert stats + list pagination correct; measure wall time.
**Acceptance:** <60s locally, all assertions green.
**Verify:** `cargo test soak -- --ignored || cargo test soak`
**Commit:** `test(contracts): soak 100 [ISSUE-052]`

### ISSUE-053: Error-message quality (every panic has code + doc link)
**Goal:** Debuggability for API mapping.
**Tasks:**
- Sweep `panic_with_error!` sites: ensure typed code (no bare `panic!`), add context where legal; update `docs/ERRORS.md` with trigger + API HTTP mapping (e.g., Unauthorized→403).
**Acceptance:** `rg "panic!" src` returns only tests/macros.
**Verify:** `rg -n "panic!" src || true`
**Commit:** `refactor(contracts): error quality [ISSUE-053]`

### ISSUE-054: No-unwrap / no-expect sweep in contract code
**Goal:** No hidden traps.
**Tasks:**
- `rg "unwrap\(\)|\.expect\(" src/` must be empty (allow tests/); replace with error propagation.
**Acceptance:** Grep clean.
**Verify:** `! rg -n "\.unwrap\(\)|\.expect\(" src/`
**Commit:** `security(contracts): no-unwrap sweep [ISSUE-054]`

### ISSUE-055: Docs polish (ARCHITECTURE, STORAGE, EVENTS, ERRORS cross-linked)
**Goal:** Single coherent spec.
**Tasks:**
- Cross-link all docs, add mermaid lifecycle diagram, storage key table, event catalog; fix dead links (`markdown-link-check` or manual).
**Acceptance:** No dead intra-links.
**Verify:** manual
**Commit:** `docs(contracts): polish crosslinks [ISSUE-055]`

### ISSUE-056: JS client tests (arg builders, error mapping) with mocked RPC
**Goal:** API team safety.
**Tasks:**
- `clients/js/*.test.ts` (vitest/jest): uuid→BytesN, stroops conversion, error map; mock fetch for RPC shape.
**Acceptance:** `npm test` in clients/ green (or documented skip if no node).
**Verify:** `npm test --prefix clients/js || echo ok`
**Commit:** `test(contracts): js client tests [ISSUE-056]`

### ISSUE-057: Reproducible build (pinned toolchain, `soroban build` hash)
**Goal:** Verifiable WASM.
**Tasks:**
- `make reproducible` records `sha256sum` of WASM + toolchain versions to `deployments/builds.json`; document verify steps.
**Acceptance:** Two consecutive builds → same hash (or documented variance).
**Verify:** `sha256sum target/wasm32*/release/*.wasm`
**Commit:** `chore(contracts): reproducible build [ISSUE-057]`

### ISSUE-058: CHANGELOG + versioning policy (semver for contract)
**Goal:** Release discipline.
**Tasks:**
- `CHANGELOG.md` (Unreleased → v1.0.0-rc1), policy: interface change = major, additive view = minor; `version()` bump procedure.
**Acceptance:** Changelog entries for all phases.
**Verify:** doc review
**Commit:** `docs(contracts): changelog policy [ISSUE-058]`

### ISSUE-059: SECURITY + disclosure + pause drill contact list template
**Goal:** Incident-ready.
**Tasks:**
- `SECURITY.md` finalize (supported versions, report email placeholder, 90-day disclosure, pause authority + key-holder template).
**Acceptance:** Complete.
**Verify:** doc review
**Commit:** `docs(contracts): security disclosure [ISSUE-059]`

### ISSUE-060: Pre-mainnet freeze review (interface lock, no new features)
**Goal:** RC1 gate.
**Tasks:**
- Freeze interface (this plan's 6 entrypoints + 2 views); any change requires ADR in `docs/adr/`; create `docs/adr/001-xlm-sac.md` + `002-storage-pagination.md` retroactively.
**Acceptance:** ADR dir with 2+ records, freeze note in README.
**Verify:** `ls docs/adr/`
**Commit:** `docs(contracts): freeze adr [ISSUE-060]`

---

## PHASE F — DEPLOY ARTIFACTS & HARDENING (061–067)

### ISSUE-061: WASM artifact + hash + optimization (release profile, lto)
**Goal:** Minimal deployable.
**Tasks:**
- `Cargo.toml` release `[profile.release] opt-level=z, lto=true, strip=true`; `make build` outputs `target/.../escrow.wasm` + `sha256`; record in `deployments/builds.json`.
**Acceptance:** Size <50KB (or documented).
**Verify:** `make build && ls -lh target/**/escrow.wasm`
**Commit:** `chore(contracts): wasm optimize [ISSUE-061]`

### ISSUE-062: `deployments/` registry (testnet.json, mainnet.json template, builds.json)
**Goal:** Single source of deployed truth.
**Tasks:**
- Schema `{network, contractId, wasmHash, admin, signer, deployedAt, txHash, version}`; validate via `scripts/validate-deployments.sh` (jq schema check).
**Acceptance:** Validator green.
**Verify:** `./scripts/validate-deployments.sh`
**Commit:** `chore(contracts): deployments registry [ISSUE-062]`

### ISSUE-063: Dockerfile (rust builder for reproducible CI builds)
**Goal:** CI builds without host variance.
**Tasks:**
- `Dockerfile` (rust:1.7x-slim → build → export wasm via `scratch`/`alpine` copy), `.dockerignore`; `make docker-build` smoke.
**Acceptance:** `docker build -t escrow:local .` succeeds.
**Verify:** `docker build -t escrow:local .`
**Commit:** `chore(contracts): dockerfile [ISSUE-063]`

### ISSUE-064: Stellar CLI version check script + network configs
**Goal:** No wrong-network deploys.
**Tasks:**
- `networks/{testnet.toml, mainnet.toml}` (rpc, horizon, passphrase, sac-xlm id), `scripts/check-network.sh` asserts RPC matches expected passphrase before deploy.
**Acceptance:** Mismatched RPC aborts deploy (test with fake URL).
**Verify:** `./scripts/check-network.sh testnet || true`
**Commit:** `feat(contracts): network configs guard [ISSUE-064]`

### ISSUE-065: Disaster recovery note (backend down → CLI manual release/refund)
**Goal:** Arch §7 funds-safe claim.
**Tasks:**
- `docs/DISASTER_RECOVERY.md`: direct CLI commands to release/refund given request_id + signer key via HSM (no raw-key encouragement), RPO/RTO, state-on-chain note.
**Acceptance:** Commands copy-paste runnable (reviewed).
**Verify:** doc review
**Commit:** `docs(contracts): disaster recovery [ISSUE-065]`

### ISSUE-066: Production checklist (audit, freeze, testnet proof, mainnet pilot)
**Goal:** Pre-release gate.
**Tasks:**
- `docs/PRODUCTION.md`: toolchain pins, full test green, audit checklist tick, gas report, testnet smoke hashes, upgrade drill ok, mainnet pilot (small-value, multisig admin, monitoring of events), rollback plan.
**Acceptance:** All ticked except live mainnet (marked pilot-pending).
**Verify:** manual review
**Commit:** `docs(contracts): production checklist [ISSUE-066]`

### ISSUE-067: License + headers + SBOM-ish (cargo tree snapshot)
**Goal:** Compliance.
**Tasks:**
- `LICENSE` (Apache-2.0/MIT dual per Soroban norm — pick + record), `cargo tree > deployments/dependencies.txt`, `cargo audit` (or `cargo deny`) clean/triaged.
**Acceptance:** `cargo audit` no unpatched critical (or documented).
**Verify:** `cargo audit || cargo deny check || true`
**Commit:** `chore(contracts): license sbom [ISSUE-067]`

---

## PHASE G — CI/CD WORKFLOWS (068–070) ★ REQUIRED

### ISSUE-068: GitHub Actions — CI (fmt, clippy, test, build)
**Goal:** Every push/PR gated.
**Tasks:**
- `.github/workflows/ci.yml`: ubuntu-latest, rust stable (pinned via rust-toolchain), cache cargo; jobs: `fmt (cargo fmt --check)`, `clippy (-D warnings)`, `test (cargo test --all-targets)`, `build (soroban build / cargo build wasm + upload escrow.wasm artifact + sha256)`. Branch protection doc `docs/BRANCH_PROTECTION.md`.
**Acceptance:** Workflow green on this commit; badge in README.
**Verify:** `cat .github/workflows/ci.yml && cargo fmt --check && cargo test`
**Commit:** `ci(contracts): github actions ci [ISSUE-068]`

### ISSUE-069: GitHub Actions — Security + testnet smoke (gated) + docker
**Goal:** Supply-chain + live confidence.
**Tasks:**
- `.github/workflows/security.yml`: `cargo audit`/`deny`, `gitleaks`, shellcheck for scripts.
- `.github/workflows/testnet.yml`: manual `workflow_dispatch` only: builds WASM, runs `smoke-testnet.sh` with secrets (`TESTNET_SOURCE_SECRET`), uploads `deployments/smoke-*.json`. Never auto-runs on PR.
- `.github/workflows/docker.yml`: on `main`+tags: build Dockerfile, push GHCR `ghcr.io/<org>/stellarrail-contracts`, attest.
**Acceptance:** Security workflow green; testnet workflow valid YAML (dispatch-only).
**Verify:** inspect YAML + `cargo test`
**Commit:** `ci(contracts): security testnet docker [ISSUE-069]`

### ISSUE-070: GitHub Actions — Release (version check, changelog, wasm release, tag RC1)
**Goal:** Production-ready delivery.
**Tasks:**
- `.github/workflows/release.yml`: on tag `contracts-v*`: verify `version()` == tag, full CI re-run, attach `escrow.wasm` + `sha256` + `deployments/builds.json` to GitHub Release with changelog excerpt, require `docs/PRODUCTION.md` tick (manual approval via environment `mainnet-gate` for mainnet deploy job — separate guarded job, default skipped).
- README badges (CI, security, release), final full green `fmt+clippy+test+build`, tag `contracts-v1.0.0-rc1`.
**Acceptance:** All workflows valid (`gh workflow list` or actionlint), badges render, tag pushed, Release draft creatable.
**Verify:** `ls .github/workflows/ && cargo fmt --check && cargo test`
**Commit:** `ci(contracts): release rc1 [ISSUE-070]`

---

## DONE DEFINITION
- [ ] 70 commits minimum, sequential.
- [ ] `cargo fmt --check`, `clippy -D warnings`, `cargo test --all-targets`, wasm build green.
- [ ] Testnet smoke + expire drill evidence (or SKIP_LIVE documented + sandbox suite green).
- [ ] `.github/workflows/{ci,security,testnet,docker,release}.yml` present + passing/valid.
- [ ] `escrow.wasm` + sha256 + deployments registry complete.
- [ ] `docs/{PRODUCTION,INDEXER,ERRORS,EVENTS,AUDIT_CHECKLIST,DISASTER_RECOVERY}.md` complete.
- [ ] Tag `contracts-v1.0.0-rc1` pushed.

*End of stellarrail-contracts build plan.*
