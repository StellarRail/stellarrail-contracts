#!/usr/bin/env bash
# Read-only queries: get-request / get-stats / list.
# Usage: ./scripts/query.sh get-request --request-id <64hex> [NETWORK_ENV...]
#        ./scripts/query.sh get-stats
#        ./scripts/query.sh list --offset 0 --limit 20
# Env: CONTRACT_ID (or .sandbox-contract-id), RPC_URL, NETWORK_PASSPHRASE.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export CONTRACT_ID="${CONTRACT_ID:-$(cat "${ROOT}/.sandbox-contract-id" 2>/dev/null || echo '')}"
export RPC_URL="${RPC_URL:-http://localhost:8000/rpc}"
export NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
SUB="${1:-help}"
shift || true
case "${SUB}" in
get-request) exec "${ROOT}/scripts/invoke.sh" get-request "$@" ;;
get-stats) exec "${ROOT}/scripts/invoke.sh" get-stats "$@" ;;
list) exec "${ROOT}/scripts/invoke.sh" list "$@" ;;
*) echo "usage: query.sh {get-request|get-stats|list} ..." >&2; exit 1 ;;
esac
