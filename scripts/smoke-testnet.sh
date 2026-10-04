#!/usr/bin/env bash
# Testnet smoke: 1 XLM deposit → release on live testnet.
#
# Evidence from the manual alpha run: deployments/smoke-2026-10-04.json.
# This script automates the same loop for re-runs and CI (dispatch only).
#
# Required env (unless SKIP_LIVE=1):
#   ADMIN_SOURCE    CLI identity of the contract admin (signs release)
# Optional env:
#   CONTRACT_ID     (default: deployments/testnet.json)
#   RPC_URL         (default https://soroban-testnet.stellar.org)
#   SKIP_LIVE=1     stub green without touching the network (CI default)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
PASSPHRASE="Test SDF Network ; September 2015"
NET_ARGS=(--rpc-url "${RPC_URL}" --network-passphrase "${PASSPHRASE}")

if [ "${SKIP_LIVE:-0}" = "1" ]; then
  echo "[smoke-testnet] SKIP_LIVE=1: stub green (no network)."
  echo "[smoke-testnet] existing evidence: $(ls "${ROOT}"/deployments/smoke-*.json 2>/dev/null || echo none)"
  exit 0
fi

: "${ADMIN_SOURCE:?set ADMIN_SOURCE (CLI identity of the contract admin)}"
CONTRACT_ID="${CONTRACT_ID:-$(jq -r .contractId "${ROOT}/deployments/testnet.json")}"
TOKEN="$(jq -r .token "${ROOT}/deployments/testnet.json")"
ADMIN_ADDR="$(jq -r .admin "${ROOT}/deployments/testnet.json")"
TS="$(date +%s)"
DEP_ALIAS="smoke-dep-${TS}"
DST_ALIAS="smoke-dst-${TS}"

echo "[smoke-testnet] contract ${CONTRACT_ID}"
stellar keys generate "${DEP_ALIAS}" --network testnet >/dev/null
stellar keys generate "${DST_ALIAS}" --network testnet >/dev/null
# Both accounts need on-chain entries (the SAC balance check traps on
# missing entries).
stellar keys fund "${DEP_ALIAS}" --network testnet >/dev/null
stellar keys fund "${DST_ALIAS}" --network testnet >/dev/null
DEP_ADDR="$(stellar keys address "${DEP_ALIAS}")"
DST_ADDR="$(stellar keys address "${DST_ALIAS}")"
echo "[smoke-testnet] depositor=${DEP_ADDR} destination=${DST_ADDR}"

tx_hash() { grep -oE 'tx/[0-9a-f]{64}' | head -1 | cut -d/ -f2; }

sac_balance() {
  stellar contract invoke --id "${TOKEN}" "${NET_ARGS[@]}" \
    --source-account "${DEP_ALIAS}" --send=no -- balance --id "$1" | jq -r .
}

CLOSE="$(curl -s -m 10 -X POST "${RPC_URL}" -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' | jq -r .result.latestLedgerCloseTime)"
REQ="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
DST_BEFORE="$(sac_balance "${DST_ADDR}")"

echo "[smoke-testnet] depositing 1 XLM..."
DEPLOY_OUT="$(CONTRACT_ID="${CONTRACT_ID}" RPC_URL="${RPC_URL}" \
  NETWORK_PASSPHRASE="${PASSPHRASE}" "${ROOT}/scripts/invoke.sh" deposit \
  --amount 10000000 --request-id "${REQ}" \
  --depositor "${DEP_ADDR}" --destination "${DST_ADDR}" \
  --deadline "$((CLOSE + 3600))" --source "${DEP_ALIAS}" 2>&1)"
echo "${DEPLOY_OUT}" | tail -2
DEP_TX="$(echo "${DEPLOY_OUT}" | tx_hash)"

echo "[smoke-testnet] releasing..."
REL_OUT="$(CONTRACT_ID="${CONTRACT_ID}" RPC_URL="${RPC_URL}" \
  NETWORK_PASSPHRASE="${PASSPHRASE}" "${ROOT}/scripts/invoke.sh" release \
  --caller "${ADMIN_ADDR}" --request-id "${REQ}" --source "${ADMIN_SOURCE}" 2>&1)"
echo "${REL_OUT}" | tail -2
REL_TX="$(echo "${REL_OUT}" | tx_hash)"

DST_AFTER="$(sac_balance "${DST_ADDR}")"
DIFF="$((DST_AFTER - DST_BEFORE))"
if [ "${DIFF}" != "10000000" ]; then
  echo "[smoke-testnet] FAIL: destination delta=${DIFF} (want 10000000)" >&2
  exit 1
fi
echo "[smoke-testnet] destination +10000000 stroops OK"

mkdir -p "${ROOT}/deployments"
OUT="${ROOT}/deployments/smoke-${TS}.json"
jq -n \
  --arg network "testnet" \
  --arg contractId "${CONTRACT_ID}" \
  --arg requestId "${REQ}" \
  --arg depositTx "${DEP_TX}" \
  --arg releaseTx "${REL_TX}" \
  --arg depositor "${DEP_ADDR}" \
  --arg destination "${DST_ADDR}" \
  --arg ranAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  '{network:$network,contractId:$contractId,requestId:$requestId,
    depositTx:$depositTx,releaseTx:$releaseTx,
    depositor:$depositor,destination:$destination,ranAt:$ranAt}' >"${OUT}"
echo "[smoke-testnet] evidence: ${OUT}"
