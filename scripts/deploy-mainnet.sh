#!/usr/bin/env bash
# Deploy the escrow contract on MAINNET. Refuses to run without an explicit
# confirmation gate + multisig ceremony.
#
# Required env:
#   CONFIRM_MAINNET=DEPLOY-TO-MAINNET   (exact string; anything else aborts)
#   SOURCE_SECRET    deployer secret (multisig signer / HSM proxy — never paste raw keys into chat)
#   ADMIN_ADDRESS    G... (multisig account)
#   SIGNER_ADDRESS   G... (HSM-backed signer)
#   TOKEN_ADDRESS    mainnet native XLM SAC id (resolved + verified out-of-band)
#   RPC_URL          mainnet RPC endpoint
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [ "${CONFIRM_MAINNET:-}" != "DEPLOY-TO-MAINNET" ]; then
  echo "ABORT: set CONFIRM_MAINNET=DEPLOY-TO-MAINNET to proceed." >&2
  echo "See docs/DEPLOY_MAINNET.md for the full pilot ceremony." >&2
  exit 1
fi
: "${SOURCE_SECRET:?set SOURCE_SECRET}"
: "${ADMIN_ADDRESS:?set ADMIN_ADDRESS}"
: "${SIGNER_ADDRESS:?set SIGNER_ADDRESS}"
: "${TOKEN_ADDRESS:?set TOKEN_ADDRESS}"
: "${RPC_URL:?set RPC_URL}"

echo "MAINNET deployment in 10s — Ctrl-C to abort."
for i in 10 9 8 7 6 5 4 3 2 1; do
  printf '\r%s...' "$i"
  sleep 1
done
printf '\n'

echo "[deploy-mainnet] building..."
stellar contract build
WASM="$(ls -t "${ROOT}"/target/wasm32v1-none/release/escrow.wasm | head -1)"
HASH="$(sha256sum "${WASM}" | cut -d' ' -f1)"
echo "[deploy-mainnet] wasm sha256=${HASH}"

EXPECTED_HASH="$(jq -r .wasmHash "${ROOT}/deployments/builds.json" 2>/dev/null || echo '')"
if [ -n "${EXPECTED_HASH}" ] && [ "${EXPECTED_HASH}" != "${HASH}" ]; then
  echo "ABORT: local wasm hash != deployments/builds.json (reproducible-build drift)." >&2
  exit 1
fi

echo "[deploy-mainnet] deploying..."
CONTRACT_ID="$(stellar contract deploy \
  --wasm "${WASM}" \
  --source "${SOURCE_SECRET}" \
  --network mainnet \
  --rpc-url "${RPC_URL}")"
echo "[deploy-mainnet] contract: ${CONTRACT_ID}"

echo "[deploy-mainnet] NEXT: initialize with the multisig ceremony, then run the"
echo "small-value pilot in docs/DEPLOY_MAINNET.md. This script never initializes"
echo "mainnet automatically — that step requires N-of-M signatures."
