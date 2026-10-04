# Testnet deployment runbook (alpha)

## Prerequisites

- Stellar CLI ≥ 27.1.0 (`stellar --version`).
- A deployer secret with a few XLM (Friendbot):
  ```bash
  stellar keys generate deployer --network testnet
  stellar keys fund deployer --network testnet
  ```
- Two role addresses (admin + signer). Test keys are fine for alpha:
  ```bash
  stellar keys generate admin --network testnet
  stellar keys generate signer --network testnet
  stellar keys fund admin --network testnet
  ```

## Deploy

```bash
export SOURCE_SECRET="deployer"          # CLI identity that deploys
export ADMIN_SECRET="admin"              # CLI identity that signs initialize
export ADMIN_ADDRESS="$(stellar keys address admin)"
export SIGNER_ADDRESS="$(stellar keys address signer)"
./scripts/deploy-testnet.sh
# → deployments/testnet.json {network, contractId, wasmHash, admin, signer,
#   deployedAt, txHash, version}
```

The script resolves the native-XLM SAC id via
`stellar contract id asset --asset native --network testnet` and passes it
to `initialize(admin, signer, token)`.

## Smoke (1 XLM)

```bash
export CONTRACT_ID="$(jq -r .contractId deployments/testnet.json)"
export RPC_URL="https://soroban-testnet.stellar.org"
export NETWORK_PASSPHRASE="Test SDF Network ; September 2015"

REQ="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
NOW="$(date +%s)"

# deposit → release (note: Option<Address> args are auto-quoted to JSON
# by scripts/invoke.sh; raw CLI use needs --destination '"G..."')
./scripts/invoke-deposit.sh --amount 10000000 --request-id "$REQ" \
  --depositor "$ADMIN_ADDRESS" --destination "$SIGNER_ADDRESS" \
  --deadline "$((NOW + 3600))" --source admin
./scripts/invoke-release.sh --caller "$ADMIN_ADDRESS" --request-id "$REQ" --source admin

# deposit → expire (deadline must clear the 60s minimum AT LEDGER TIME:
# derive it from the latest close time, not wall clock)
CLOSE="$(curl -s -X POST "$RPC_URL" -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' | jq .result.latestLedgerCloseTime)"
REQ2="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
./scripts/invoke-deposit.sh --amount 10000000 --request-id "$REQ2" \
  --depositor "$ADMIN_ADDRESS" --destination "$SIGNER_ADDRESS" \
  --deadline "$((CLOSE + 75))" --source admin
sleep 100
./scripts/invoke-expire.sh --request-id "$REQ2" --source signer
./scripts/query.sh get-stats --source admin
```

The automated version of this loop is `scripts/smoke-testnet.sh`
(ISSUE-048); it records hashes to `deployments/smoke-*.json`.

## Verify on explorer

- Contract: `https://stellar.expert/explorer/testnet/contract/<CONTRACT_ID>`
- Transactions: look up each invoke's tx hash from the CLI output.

## Record

Paste the contract id + tx hashes into `deployments/testnet.json`
(`txHash`) and link them in the alpha sign-off. Never commit secrets —
only ids and hashes.
