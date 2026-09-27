# Indexer guide (for the API team)

Poll Soroban events and mirror escrow state into the API database.
Event catalog: `docs/EVENTS.md`. Auth model: `docs/AUTH_MATRIX.md`.

## What to index

| Goal | Source |
|---|---|
| New escrow | `funds_locked` → insert row (`request_id`, `depositor`, `amount`, `deadline` unknown — fetch via `get_request`) |
| Settlement | `payment_released` / `payment_refunded` / `request_expired` → mark terminal |
| Roles | `initialized`, `signer_updated`, `admin_transfer_*`, `paused`/`unpaused` → config audit log |
| Reconciliation | `get_stats` + `list_requests` pages vs DB aggregates |

`deadline` is NOT in the event payload (only `amount`, `actor`,
`ledger_time`); fetch it once via `get_request` after `funds_locked`.

## Polling with RPC `getEvents`

```bash
curl -s -X POST https://soroban-testnet.stellar.org \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0", "id": 1, "method": "getEvents",
    "params": {
      "startLedger": 8123000,
      "filters": [{
        "type": "contract",
        "contractIds": ["<ESCROW_CONTRACT_ID>"],
        "topics": [["funds_locked", "*"], ["payment_released", "*"],
                   ["payment_refunded", "*"], ["request_expired", "*"]]
      }],
      "pagination": {"limit": 100}
    }
  }'
```

Walk `cursor` forward; persist it. One filter per event name also works
(single-topic filters are cheaper to eyeball, same cost).

With the CLI:

```bash
stellar events get --rpc-url https://soroban-testnet.stellar.org \
  --start-ledger <CURSOR> --output json --limit 100
```

## TypeScript sketch (`stellar-sdk`)

```ts
import { rpc } from "@stellar/stellar-sdk";

const server = new rpc.Server("https://soroban-testnet.stellar.org");

for await (const page of poll(server, contractId, cursor)) {
  for (const ev of page.events) {
    const [name, requestId] = ev.topic as [string, string];
    // name: "funds_locked" | "payment_released" | ...
    // requestId: 64-hex request id; data: { amount, actor, ledger_time }
    await upsert(name, requestId, ev);
    cursor = ev.cursor; // persist AFTER processing
  }
}
```

`BytesN<32>` ↔ 64-hex: `Buffer.from(requestId, "hex")`; uuid → bytes via
`uuid.parse()` then hex (see `clients/js/`).

## Correctness rules

1. **Idempotency:** dedupe on `(tx_hash, event_index)` — RPC redelivers.
   DB upserts must be keyed by `request_id`, never append-only.
2. **Reorgs:** act only after 10 ledgers of finality (testnet AND mainnet).
   Keep `cursor` 10 behind the tip.
3. **Terminal states are final:** `Released`/`Refunded`/`Expired` never
   transition — a second event for the same id is a bug alarm, not an update.
4. **No-event means no mutation:** reverted invocations publish nothing, so
   never infer state from attempted transactions — only from events.
5. **Filtered `list_requests` scans walk pages** (`offset`/`limit` slice the
   raw registry; see `get_request`/`list_requests` docs) until a short page.
6. **Clock skew:** treat escrows as expired at `deadline + 300s`, and submit
   downstream actions with margin (`docs/STORAGE.md`).
