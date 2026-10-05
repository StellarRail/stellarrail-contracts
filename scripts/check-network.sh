#!/usr/bin/env bash
# Assert the RPC at $RPC_URL serves the expected network before deploying.
# Usage: ./scripts/check-network.sh testnet|mainnet
# Exits non-zero (aborting the caller) on any mismatch.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NET="${1:?usage: check-network.sh testnet|mainnet}"
CFG="${ROOT}/networks/${NET}.toml"
[ -f "${CFG}" ] || { echo "unknown network: ${NET}" >&2; exit 1; }

EXPECTED_URL="$(grep -E '^rpc_url' "${CFG}" | cut -d'"' -f2)"
EXPECTED_PHRASE="$(grep -E '^network_passphrase' "${CFG}" | cut -d'"' -f2)"
RPC_URL="${RPC_URL:-${EXPECTED_URL}}"

echo "[check-network] ${NET}: rpc=${RPC_URL}"
INFO="$(curl -fsS -m 15 -X POST "${RPC_URL}" \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getNetwork"}' 2>/dev/null || echo '')"
if [ -z "${INFO}" ]; then
  echo "[check-network] WARN: RPC unreachable; cannot verify (refusing to deploy blind)." >&2
  exit 1
fi
ACTUAL_PHRASE="$(echo "${INFO}" | jq -r .result.passphrase 2>/dev/null || echo '')"
if [ "${ACTUAL_PHRASE}" != "${EXPECTED_PHRASE}" ]; then
  echo "[check-network] ABORT: passphrase mismatch." >&2
  echo "  expected: ${EXPECTED_PHRASE}" >&2
  echo "  actual:   ${ACTUAL_PHRASE}" >&2
  exit 1
fi
echo "[check-network] OK: passphrase matches (${NET})."
