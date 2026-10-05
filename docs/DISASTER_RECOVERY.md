# Disaster recovery (backend down → funds stay safe)

Core claim: **escrow state and funds live on-chain**. If the entire
StellarRail backend disappears, every escrow can still be settled directly
with the Stellar CLI + the admin/signer keys. No database, no API, no
indexer is required to move funds.

## State-on-chain recap

- Requests: `Request(request_id)` persistent entries (survive backend loss).
- Roles: `Admin` / `Signer` entries. Keys live with their holders
  (multisig / HSM at mainnet), never in the backend.
- Money: SAC XLM held by the contract account itself.

## Direct CLI settlement (no API)

```bash
export CONTRACT_ID="<from deployments/<net>.json>"
export RPC_URL="<network rpc>" NETWORK_PASSPHRASE="<passphrase>"

# Inspect (read-only, any funded key as --source):
./scripts/query.sh get-request --request-id <64hex> --source <any-key>
./scripts/query.sh list --offset 0 --limit 50 --source <any-key>

# Release to destination (admin OR signer key required):
./scripts/invoke-release.sh --caller <ADMIN_OR_SIGNER_G...> \
  --request-id <64hex> --source <admin-or-signer-identity>

# Refund to depositor (admin OR signer key required):
./scripts/invoke-refund.sh --caller <ADMIN_OR_SIGNER_G...> \
  --request-id <64hex> --source <admin-or-signer-identity>

# Rescue past-due escrow (ANY funded key — no role needed):
./scripts/invoke-expire.sh --request-id <64hex> --source <any-identity>
```

Key handling: prefer hardware signers / multisig ceremony. Never paste raw
secret keys into chat, tickets, or shared docs — sign locally with CLI
identities (`stellar keys`) or an HSM proxy.

## Scenarios

| Scenario | Response |
|---|---|
| API/indexer down, chain fine | Settle via CLI above; `expire` needs no privileged key at all. |
| Admin key lost, signer alive | Signer settles everything (`release`/`refund`); rotate admin later via… admin-gated `transfer_admin` — if admin is unrecoverable, `migrate` is also admin-gated: **protect the admin key with multisig** (see `docs/DEPLOY_MAINNET.md`). |
| Active exploit / suspicious settlement | `set_paused(true)` immediately (pause drill contacts in `SECURITY.md`); funds remain rescuable via `expire` while paused. |
| Contract bug | Fix → rebuild → `migrate` per `docs/UPGRADE.md`; state preserved. |
| Chain halt / RPC outage | Wait; escrows accrue no penalty for lateness except their own deadlines, and `expire` only helps the depositor. RPO = last ledger; RTO = chain recovery + keeper catch-up. |

## Keeper minimum (keeps `expire` timely)

At least one independent cron/keeper must, every 10 minutes:

1. `list_requests` walk (pages of 50) for `Locked` requests,
2. `expire` any with `deadline + 300s < now`,
3. alert on any `Locked` request older than 7 days without settlement.

Without keepers, dormant escrows approach the 30d TTL edge
(`docs/STORAGE.md` §Eviction) — never let an escrow sit past-due.
