#!/usr/bin/env bash
# Deploy + initialize the escrow contract on TESTNET.
#
# Required env:
#   SOURCE_SECRET   secret key or CLI identity of the deployer (Friendbot-funded)
#   ADMIN_SECRET    secret key or CLI identity of the admin (signs initialize;
#                   fund with a little XLM for fees)
#   ADMIN_ADDRESS   G... admin
#   SIGNER_ADDRESS  G... signer
# Optional env:
#   TOKEN_ADDRESS   SAC XLM id (default: resolved via `stellar contract id asset`)
#   RPC_URL         (default https://soroban-testnet.stellar.org)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
PASSPHRASE="Test SDF Network ; September 2015"

: "${SOURCE_SECRET:?set SOURCE_SECRET (deployer secret, Friendbot-funded)}"
: "${ADMIN_SECRET:?set ADMIN_SECRET (admin signer for initialize)}"
: "${ADMIN_ADDRESS:?set ADMIN_ADDRESS}"
: "${SIGNER_ADDRESS:?set SIGNER_ADDRESS}"

echo "[deploy-testnet] building..."
stellar contract build
WASM="$(ls -t "${ROOT}"/target/wasm32v1-none/release/escrow.wasm | head -1)"
HASH="$(sha256sum "${WASM}" | cut -d' ' -f1)"
echo "[deploy-testnet] wasm ${WASM} sha256=${HASH}"

if [ -z "${TOKEN_ADDRESS:-}" ]; then
  TOKEN_ADDRESS="$(stellar contract id asset --asset native --network testnet)"
  echo "[deploy-testnet] resolved native XLM SAC: ${TOKEN_ADDRESS}"
fi

echo "[deploy-testnet] deploying..."
CONTRACT_ID="$(stellar contract deploy \
  --wasm "${WASM}" \
  --source "${SOURCE_SECRET}" \
  --network testnet \
  --network-passphrase "${PASSPHRASE}" \
  --rpc-url "${RPC_URL}")"
echo "[deploy-testnet] contract: ${CONTRACT_ID}"

echo "[deploy-testnet] initializing (signed by ADMIN)..."
stellar contract invoke \
  --id "${CONTRACT_ID}" \
  --source "${ADMIN_SECRET}" \
  --network testnet \
  --network-passphrase "${PASSPHRASE}" \
  --rpc-url "${RPC_URL}" \
  -- initialize --admin "${ADMIN_ADDRESS}" --signer "${SIGNER_ADDRESS}" --token "${TOKEN_ADDRESS}"

mkdir -p "${ROOT}/deployments"
DEPLOYED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
jq -n \
  --arg network "testnet" \
  --arg contractId "${CONTRACT_ID}" \
  --arg wasmHash "${HASH}" \
  --arg admin "${ADMIN_ADDRESS}" \
  --arg signer "${SIGNER_ADDRESS}" \
  --arg token "${TOKEN_ADDRESS}" \
  --arg deployedAt "${DEPLOYED_AT}" \
  --arg version "1" \
  '{network:$network,contractId:$contractId,wasmHash:$wasmHash,admin:$admin,
    signer:$signer,token:$token,deployedAt:$deployedAt,txHash:"",version:($version|tonumber)}' \
  >"${ROOT}/deployments/testnet.json"
echo "[deploy-testnet] registry: deployments/testnet.json"
echo "[deploy-testnet] smoke: ./scripts/smoke-testnet.sh (ISSUE-048)"
