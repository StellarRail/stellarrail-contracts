#!/usr/bin/env bash
# Local sandbox: build the WASM and, when a local Stellar RPC is reachable,
# deploy + initialize the escrow contract for the fast inner loop.
#
# Env knobs:
#   RPC_URL        local RPC endpoint (default http://localhost:8000/rpc)
#   ADMIN_SECRET   secret key of the local admin (default: sandbox identity)
#   SKIP_DEPLOY=1  build only, never touch the network
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RPC_URL="${RPC_URL:-http://localhost:8000/rpc}"

echo "[sandbox] building wasm..."
stellar contract build

WASM="$(ls -t "${ROOT}"/target/wasm32v1-none/release/escrow.wasm | head -1)"
echo "[sandbox] wasm: ${WASM} ($(wc -c <"${WASM}") bytes)"

if [ "${SKIP_DEPLOY:-0}" = "1" ]; then
  echo "[sandbox] SKIP_DEPLOY=1: build only."
  exit 0
fi

if ! curl -fsS -m 3 -X POST "${RPC_URL}" \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' >/dev/null 2>&1; then
  echo "[sandbox] no RPC at ${RPC_URL}."
  echo "[sandbox] start one (docker): stellar network start --limits unlimited  # or"
  echo "[sandbox] run unit tests instead: cargo test"
  echo "[sandbox] build OK, deploy skipped (exit 0)."
  exit 0
fi

echo "[sandbox] deploying to local network (${RPC_URL})..."
if [ -z "${ADMIN_SECRET:-}" ]; then
  stellar keys generate --no-fund admin --network local 2>/dev/null || true
  ADMIN_SECRET="$(stellar keys show admin 2>/dev/null || echo '')"
fi
: "${ADMIN_SECRET:?set ADMIN_SECRET or let the script generate the admin key}"

CONTRACT_ID="$(stellar contract deploy \
  --wasm "${WASM}" \
  --source "${ADMIN_SECRET}" \
  --rpc-url "${RPC_URL}" \
  --network-passphrase 'Standalone Network ; February 2017')"
echo "[sandbox] contract: ${CONTRACT_ID}"
echo "${CONTRACT_ID}" >"${ROOT}/.sandbox-contract-id"

echo "[sandbox] done. Invoke via ./scripts/invoke.sh (see docs/LOCAL_DEV.md)."
