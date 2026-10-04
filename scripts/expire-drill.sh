#!/usr/bin/env bash
# Testnet expire drill: short-deadline deposit, wait, permissionless expire
# by a THIRD PARTY (not admin/signer/depositor), asserting full refund.
#
# Evidence from the manual alpha run: deployments/smoke-2026-10-04.json
# (request 594a…803b, expire tx f211c9d1…).
#
# Required env (unless SKIP_LIVE=1): none beyond funded Friendbot access.
# Optional env:
#   CONTRACT_ID  (default: deployments/testnet.json)
#   RPC_URL      (default https://soroban-testnet.stellar.org)
#   SKIP_LIVE=1  stub green without touching the network (CI default)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
PASSPHRASE="Test SDF Network ; September 2015"
NET_ARGS=(--rpc-url "${RPC_URL}" --network-passphrase "${PASSPHRASE}")

if [ "${SKIP_LIVE:-0}" = "1" ]; then
  echo "[expire-drill] SKIP_LIVE=1: stub green (no network)."
  exit 0
fi

CONTRACT_ID="${CONTRACT_ID:-$(jq -r .contractId "${ROOT}/deployments/testnet.json")}"
TOKEN="$(jq -r .token "${ROOT}/deployments/testnet.json")"
TS="$(date +%s)"
DEP_ALIAS="exp-dep-${TS}"
CALLER_ALIAS="exp-caller-${TS}"

stellar keys generate "${DEP_ALIAS}" --network testnet >/dev/null
stellar keys generate "${CALLER_ALIAS}" --network testnet >/dev/null
stellar keys fund "${DEP_ALIAS}" --network testnet >/dev/null
stellar keys fund "${CALLER_ALIAS}" --network testnet >/dev/null
DEP_ADDR="$(stellar keys address "${DEP_ALIAS}")"
echo "[expire-drill] depositor=${DEP_ADDR} third-party=${CALLER_ALIAS}"

sac_balance() {
  stellar contract invoke --id "${TOKEN}" "${NET_ARGS[@]}" \
    --source-account "${DEP_ALIAS}" --send=no -- balance --id "$1" | jq -r .
}

# Deadline anchored to LEDGER close time (+75s ≈ 60s effective lifetime,
# clearing the 60s minimum even if the deposit lands a ledger or two late).
CLOSE="$(curl -s -m 10 -X POST "${RPC_URL}" -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' | jq -r .result.latestLedgerCloseTime)"
REQ="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
DEP_BEFORE="$(sac_balance "${DEP_ADDR}")"

echo "[expire-drill] depositing 1 XLM with ~60s lifetime..."
DEP_OUT="$(CONTRACT_ID="${CONTRACT_ID}" RPC_URL="${RPC_URL}" \
  NETWORK_PASSPHRASE="${PASSPHRASE}" "${ROOT}/scripts/invoke.sh" deposit \
  --amount 10000000 --request-id "${REQ}" \
  --depositor "${DEP_ADDR}" --destination "${DEP_ADDR}" \
  --deadline "$((CLOSE + 75))" --source "${DEP_ALIAS}" 2>&1)"
DEP_TX="$(echo "${DEP_OUT}" | grep -oE 'tx/[0-9a-f]{64}' | head -1 | cut -d/ -f2)"
echo "[expire-drill] deposit tx: ${DEP_TX}"

echo "[expire-drill] waiting past the deadline..."
sleep 100

echo "[expire-drill] expiring from third party..."
EXP_OUT="$(CONTRACT_ID="${CONTRACT_ID}" RPC_URL="${RPC_URL}" \
  NETWORK_PASSPHRASE="${PASSPHRASE}" "${ROOT}/scripts/invoke.sh" expire \
  --request-id "${REQ}" --source "${CALLER_ALIAS}" 2>&1)"
echo "${EXP_OUT}" | tail -1
EXP_TX="$(echo "${EXP_OUT}" | grep -oE 'tx/[0-9a-f]{64}' | head -1 | cut -d/ -f2)"

DEP_AFTER="$(sac_balance "${DEP_ADDR}")"
# The 10M escrow must come back; only tx fees may be missing. Bound 0.5 XLM:
# observed fee ~0.12 XLM, while a missing refund would lose ≥ 10 XLM-ish.
LOSS="$((DEP_BEFORE - DEP_AFTER))"
if [ "${LOSS}" -lt 0 ] || [ "${LOSS}" -gt 5000000 ]; then
  echo "[expire-drill] FAIL: depositor loss=${LOSS} stroops (want 0..5000000)" >&2
  exit 1
fi
echo "[expire-drill] escrow refunded OK (fees ${LOSS} stroops)"

STATUS="$(CONTRACT_ID="${CONTRACT_ID}" RPC_URL="${RPC_URL}" \
  NETWORK_PASSPHRASE="${PASSPHRASE}" "${ROOT}/scripts/invoke.sh" get-request \
  --request-id "${REQ}" --source "${DEP_ALIAS}" | jq -r .status)"
if [ "${STATUS}" != "4" ]; then
  echo "[expire-drill] FAIL: status=${STATUS} (want 4=Expired)" >&2
  exit 1
fi
echo "[expire-drill] status Expired OK"

mkdir -p "${ROOT}/deployments"
OUT="${ROOT}/deployments/expire-drill-${TS}.json"
jq -n \
  --arg network "testnet" \
  --arg contractId "${CONTRACT_ID}" \
  --arg requestId "${REQ}" \
  --arg depositTx "${DEP_TX}" \
  --arg expireTx "${EXP_TX}" \
  --arg depositor "${DEP_ADDR}" \
  --arg thirdParty "${CALLER_ALIAS}" \
  --arg ranAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  '{network:$network,contractId:$contractId,requestId:$requestId,
    depositTx:$depositTx,expireTx:$expireTx,
    depositor:$depositor,thirdParty:$thirdParty,ranAt:$ranAt}' >"${OUT}"
echo "[expire-drill] evidence: ${OUT}"
