# Storage schema + TTL strategy

## Keys (`src/storage.rs :: DataKey`)

| Key | Type | Notes |
|---|---|---|
| `Request(request_id)` | `EscrowRequest` | one entry per escrow; TTL extended on every write |
| `Admin` | `Address` | set by `initialize`; moved only via two-step rotation |
| `Signer` | `Address` | second release/refund authority; rotated by admin |
| `Token` | `Address` | SAC XLM contract id accepted for settlement |
| `Paused` | `bool` | emergency stop; blocks deposit/release/refund, never `expire` |
| `AllIds` | `Vec<BytesN<32>>` | append-only registry backing `list_requests` pagination |
| `Stats` | `Stats` | O(1) counters backing `get_stats` |
| `PendingAdmin` | `Address` | staged admin until `accept_admin` |

All values live in **persistent** storage (escrows must survive weeks).
No `temporary` storage is used: nothing here is safe to auto-evict.

## TTL policy

```text
PERSISTENT_TTL_THRESHOLD = 120_960 ledgers (~7d)
PERSISTENT_EXTEND_TO     = 518_400 ledgers (~30d, == MAX_DEADLINE_SECS)
```

- **Every write extends TTL** through the helpers in `src/storage.rs`
  (`set_request`, `set_admin`, `set_paused`, ...). There is no raw
  `storage().set()` call in entrypoint code.
- `518_400` matches the maximum escrow lifetime, so a live escrow cannot be
  evicted before its deadline as long as each mutation extends — and every
  mutation does.
- **Views never bump TTL** (`get_request`/`list_requests`/`get_stats` are
  strictly read-only), keeping queries cheap and side-effect free.

## Why `AllIds` instead of ledger scan

`list_requests` paginates an explicit id registry instead of scanning the
ledger: scans are unbounded (DoS/gas risk) and unavailable from inside the
contract. Trade-off: `deposit` pays one extra write + one `Vec` push. The
registry is append-only; terminal requests stay listed (filter by status).

## Clock-skew note for the API

Deadlines compare against `env.ledger().timestamp()` (close time, ~5s
granularity). The API should treat an escrow as expired only after
`deadline + 300s` grace to absorb indexer/RPC lag, and should submit
`expire` with a margin before downstream timeouts.

## Eviction + recovery

Entries live `PERSISTENT_EXTEND_TO` (30d) from their **last write**. A fully
dormant escrow (no mutation for 30d) can therefore be evicted: `get_request`
then returns `NotFound`, and `expire` cannot settle it because the record —
including the depositor address — is gone. The funds themselves are NOT lost:
they remain in the contract's SAC balance, but v1 has no function to move
funds without a request record.

Operational rule: **keeper bots must `expire` past-due escrows promptly**
(days, not weeks, after the deadline). Typical escrows live hours-to-days,
leaving weeks of TTL grace; only a ~30d-max escrow left completely
untouched approaches the edge.

If eviction ever strands funds, recovery is a contract upgrade (state is
preserved across `migrate`) shipping a one-off admin sweep keyed off SAC
balance deltas, recorded as an ADR. This is accepted residual risk for v1 —
see `docs/AUDIT_CHECKLIST.md` and `docs/PRODUCTION.md` (forthcoming,
ISSUE-066; keeper monitoring).
