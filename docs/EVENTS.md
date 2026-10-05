# Events (indexer contract)

Every mutation emits exactly one typed event. The API worker polls
`getEvents` filtered by `contractId` + topics below. Polling loop,
idempotency, and reorg rules: `docs/INDEXER.md`.

## Catalog

Topics are `[event_name_snake_case, request_id]` (the name is the
`#[contractevent]` default prefix topic). Data is a map
`{amount, actor, ledger_time}` — except `initialized`.

| Event | Topics | Data fields | Emitted by |
|---|---|---|---|
| `funds_locked` | `["funds_locked", request_id: Bytes32]` | `amount: i128` (stroops), `actor: Address` (depositor), `ledger_time: u64` | `deposit` |
| `payment_released` | `["payment_released", request_id]` | `amount`, `actor` (admin/signer caller), `ledger_time` | `release` |
| `payment_refunded` | `["payment_refunded", request_id]` | `amount`, `actor` (admin/signer caller), `ledger_time` | `refund` |
| `request_expired` | `["request_expired", request_id]` | `amount`, `actor` (caller that ran `expire`), `ledger_time` | `expire` |
| `initialized` | `["initialized", admin: Address]` | `signer: Address`, `token: Address` (SAC XLM) | `initialize` |

## RPC filter example

```bash
stellar events get \
  --rpc-url https://soroban-testnet.stellar.org \
  --start-ledger <FIRST_LEDGER> \
  --output json \
  | jq '.[] | select(.contract_id == "<ESCROW_CONTRACT_ID>")'
```

Topic filter (XDR JSON): `topics: [["funds_locked", "*"]]` scopes to
deposits; replace with the other names for settlement legs.

## Example JSON (indexer row)

```json
{
  "contract_id": "CB...ESCROW",
  "topic": ["funds_locked", "9f2c...32bytes..."],
  "data": {
    "amount": 10000000,
    "actor": "GB...DEPOSITOR",
    "ledger_time": 1788888888
  },
  "ledger": 8123456,
  "tx_hash": "abc123..."
}
```

## Indexer rules

- **Idempotency:** dedupe on `(tx_hash, event_index)` — re-delivery happens.
- **Reorgs:** act only after N ledgers of finality (testnet: 10; mainnet: 10).
- **Terminal states are final:** `Released/Refunded/Expired` never transition.
- **No-event on failure:** reverted invocations publish nothing (asserted in
  `test::events_suite`), so absence of an event means absence of mutation.
