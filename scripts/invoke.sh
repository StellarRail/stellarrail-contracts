#!/usr/bin/env bash
# Thin wrappers around `stellar contract invoke` for the escrow entrypoints.
#
# Usage:
#   ./scripts/invoke.sh deposit  --amount 10000000 --request-id <64hex> \
#       --depositor <G...> --destination <G...> --deadline <unix> \
#       --source <secret-or-key> [--contract-id <C...>]
#   ./scripts/invoke.sh release  --caller <G...> --request-id <64hex> ...
#   ./scripts/invoke.sh refund   --caller <G...> --request-id <64hex> ...
#   ./scripts/invoke.sh expire   --request-id <64hex> ...
#   ./scripts/invoke.sh get-request --request-id <64hex> ...
#   ./scripts/invoke.sh get-stats ...
#   ./scripts/invoke.sh list --offset 0 --limit 20 [--status locked]
#
# Env knobs: RPC_URL, NETWORK_PASSPHRASE, CONTRACT_ID (or .sandbox-contract-id).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RPC_URL="${RPC_URL:-http://localhost:8000/rpc}"
PASSPHRASE="${NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
CONTRACT_ID="${CONTRACT_ID:-$(cat "${ROOT}/.sandbox-contract-id" 2>/dev/null || echo '')}"
: "${CONTRACT_ID:?set CONTRACT_ID or deploy first (make deploy-local)}"

CMD="${1:-help}"
shift || true

invoke() {
  # $1 = fn, rest = stellar args
  local fn="$1"
  shift
  stellar contract invoke \
    --id "${CONTRACT_ID}" \
    --rpc-url "${RPC_URL}" \
    --network-passphrase "${PASSPHRASE}" \
    -- "$fn" "$@"
}

get() {
  local key="$1"
  shift
  for a in "$@"; do
    case "${a}" in
    "${key}") echo "${2:-}" ;;
    esac
    shift || true
  done
}

case "${CMD}" in
deposit)
  # args: --amount --request-id --depositor --destination --deadline --source
  invoke deposit \
    --amount "$(get --amount "$@")" \
    --request-id "$(get --request-id "$@")" \
    --depositor "$(get --depositor "$@")" \
    --destination "$(get --destination "$@")" \
    --deadline "$(get --deadline "$@")" \
    --source-account "$(get --source "$@")"
  ;;
release)
  invoke release \
    --caller "$(get --caller "$@")" \
    --request-id "$(get --request-id "$@")" \
    --source-account "$(get --source "$@")"
  ;;
refund)
  invoke refund \
    --caller "$(get --caller "$@")" \
    --request-id "$(get --request-id "$@")" \
    --source-account "$(get --source "$@")"
  ;;
expire)
  invoke expire \
    --request-id "$(get --request-id "$@")" \
    --source-account "$(get --source "$@")"
  ;;
get-request)
  invoke get_request --request-id "$(get --request-id "$@")"
  ;;
get-stats)
  invoke get_stats
  ;;
list)
  invoke list_requests \
    --offset "$(get --offset "$@" || echo 0)" \
    --limit "$(get --limit "$@" || echo 20)"
  ;;
*)
  sed -n '2,20p' "$0"
  exit 1
  ;;
esac
