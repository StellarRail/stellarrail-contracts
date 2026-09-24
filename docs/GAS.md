# Gas + footprint notes

WASM (release, `stellar contract build`, soroban-sdk 28.0.0):

- size: **19 KiB** (`target/wasm32v1-none/release/escrow.wasm`)
- sha256: `ff7ba0d0b61cab16d5c83a1d957c9020898b3ab5c0df5e3a12fed36fb7c0c134`
- profile: `opt-level=z, lto=true, strip=true` (root `Cargo.toml`)

## Per-entrypoint ledger access

Counts are persistent-storage reads/writes on the escrow contract itself
(SAC token legs cost their own budget on top).

| Entrypoint | Reads | Writes (+TTL extend) | Events | Notes |
|---|---|---|---|---|
| `initialize` | 1 (`Admin` exists?) | 5 (`Admin`, `Signer`, `Token`, `Paused`, `Stats`) | 1 | once per contract |
| `deposit` | 4 (`Paused`, `Request`, `Token`, `Stats`) | 3 (`Request`, `AllIds`, `Stats`) + 1 token pull | 1 | `AllIds` push grows 1 entry |
| `release` | 5 (`Paused`, `Request`, `Admin`, `Signer`, `Token`) | 2 (`Request`, `Stats`) + 1 token push | 1 | |
| `refund` | 5 (same as release) | 2 (`Request`, `Stats`) + 1 token push | 1 | |
| `expire` | 2 (`Request`, `Token`) | 2 (`Request`, `Stats`) + 1 token push | 1 | cheapest mutation; no auth metering |
| `get_request` | 1 | 0 | 0 | no TTL bump by design |
| `list_requests` | ≤ `limit` + 1 | 0 | 0 | hard-capped below |
| `get_stats` | 1 | 0 | 0 | O(1) counters, never a scan |
| `set_paused` | 1 (`Admin`) | 1 (`Paused`) | 1 | |
| `transfer_admin` / `accept_admin` | 1–2 | 1–2 | 1 | two txs total |
| `set_signer` | 2 | 1 | 1 | |
| `migrate` | 1 (`Admin`) | code update | 0 | |
| `version` | 0 | 0 | 0 | const return |

## Unbounded-loop audit

- The only loop in the contract is `list_requests`, bounded by `limit ≤ 50`
  reads (`list_requests_rejects_over_cap` asserts limit 1000 is rejected).
  `offset` cannot force extra reads: the window is `[offset, offset+limit)`.
- `track_request_id` scans `AllIds` in memory for dedupe — O(n) host-side
  iteration, no extra ledger reads, and `deposit` remains the only writer.
  If the registry ever makes deposits measurably expensive, the documented
  migration is a bump allocator (`Counter → request_id`) plus keeping
  `AllIds` for reads; see `docs/adr/002-storage-pagination.md` (ISSUE-060).
- No recursion, no dynamic dispatch over user input, no `Vec` growth tied
  to caller-controlled lengths except the capped page buffer.

## TTL auto-extend audit

Every helper in `src/storage.rs` extends TTL on write
(`PERSISTENT_EXTEND_TO` = 518 400 ledgers ≈ 30d ≥ `MAX_DEADLINE_SECS`), so:

- a live escrow cannot be evicted while its deadline is in the future;
- views never extend (read-only, cheap);
- `cargo test ttl` asserts the exact extended TTL and survival to the last
  ledger of the window (`storage::test::{writes_extend_ttl_to_policy_window,
  entry_survives_to_end_of_extended_window, singleton_writes_extend_ttl}`).

Testnet-measured numbers (per-ledger resource costs) are recorded here after
ISSUE-051 (`make report-gas`).

## Optimization pass (ISSUE-037)

Reviewed all entrypoints for clone/copy waste, early returns, and loop caps:

- `require_admin_or_signer` now returns `()` — callers only needed the gate,
  saving one `Address` clone per `release`/`refund`.
- Error returns precede every storage read (`Paused` → amount → existence →
  deadline → auth), so invalid calls pay minimum rent.
- `list_requests` loads at most `limit` requests; the offset window cannot
  force extra reads.
- No `clone()` remains that isn't required by ownership (request structs,
  event payloads, and stored values each have distinct owners).

`cargo clippy -- -W clippy::pedantic` and `-W clippy::perf` are both clean
(pedantic notes: `needless_pass_by_value` allowed crate-wide — Soroban
signatures mandate by-value params; see `src/lib.rs`).

WASM size: **19 715 bytes** after the full contract (vs 19 KiB scaffold-era
snapshot — growth is the Phase B interface itself, well under the 50 KiB
budget in ISSUE-061).
