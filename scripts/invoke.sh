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

# Parse --key value pairs into variables named OPT_<key with - → _>.
AMOUNT=""; REQUEST_ID=""; DEPOSITOR=""; DESTINATION=""; DEADLINE=""
CALLER=""; SOURCE=""; OFFSET="0"; LIMIT="20"; STATUS_FILTER=""; CONTRACT_OVERRIDE=""
while [ $# -gt 0 ]; do
  case "${1}" in
  --amount) AMOUNT="${2:?}"; shift 2 ;;
  --request-id) REQUEST_ID="${2:?}"; shift 2 ;;
  --depositor) DEPOSITOR="${2:?}"; shift 2 ;;
  --destination) DESTINATION="${2:?}"; shift 2 ;;
  --deadline) DEADLINE="${2:?}"; shift 2 ;;
  --caller) CALLER="${2:?}"; shift 2 ;;
  --source) SOURCE="${2:?}"; shift 2 ;;
  --offset) OFFSET="${2:?}"; shift 2 ;;
  --limit) LIMIT="${2:?}"; shift 2 ;;
  --status) STATUS_FILTER="${2:?}"; shift 2 ;;
  --contract-id) CONTRACT_OVERRIDE="${2:?}"; shift 2 ;;
  *) echo "unknown arg: ${1}" >&2; exit 1 ;;
  esac
done
if [ -n "${CONTRACT_OVERRIDE}" ]; then CONTRACT_ID="${CONTRACT_OVERRIDE}"; fi

# Every call needs a source identity (views use --send=no: simulate only).
need_source() {
  if [ -z "${SOURCE}" ]; then
    echo "missing --source for '${CMD}'" >&2
    exit 1
  fi
}
need_source

invoke() {
  # $1 = fn; remaining args = contract args (already split).
  local fn="$1"
  shift
  if [ -n "${IS_VIEW:-}" ]; then
    stellar contract invoke \
      --id "${CONTRACT_ID}" \
      --rpc-url "${RPC_URL}" \
      --network-passphrase "${PASSPHRASE}" \
      --source-account "${SOURCE}" \
      --send=no \
      -- "$fn" "$@"
  else
    stellar contract invoke \
      --id "${CONTRACT_ID}" \
      --rpc-url "${RPC_URL}" \
      --network-passphrase "${PASSPHRASE}" \
      --source-account "${SOURCE}" \
      -- "$fn" "$@"
  fi
}

case "${CMD}" in
deposit)
  : "${AMOUNT:?--amount required}"
  : "${REQUEST_ID:?--request-id required}"
  : "${DEPOSITOR:?--depositor required}"
  : "${DESTINATION:?--destination required}"
  : "${DEADLINE:?--deadline required}"
  # The CLI parses Option<Address> as JSON: auto-quote a bare strkey.
  case "${DESTINATION}" in
  '"'*'"') ;;
  *) DESTINATION="\"${DESTINATION}\"" ;;
  esac
  invoke deposit \
    --amount "${AMOUNT}" \
    --request-id "${REQUEST_ID}" \
    --depositor "${DEPOSITOR}" \
    --destination "${DESTINATION}" \
    --deadline "${DEADLINE}"
  ;;
release)
  : "${CALLER:?--caller required}"
  : "${REQUEST_ID:?--request-id required}"
  invoke release --caller "${CALLER}" --request-id "${REQUEST_ID}"
  ;;
refund)
  : "${CALLER:?--caller required}"
  : "${REQUEST_ID:?--request-id required}"
  invoke refund --caller "${CALLER}" --request-id "${REQUEST_ID}"
  ;;
expire)
  : "${REQUEST_ID:?--request-id required}"
  invoke expire --request-id "${REQUEST_ID}"
  ;;
get-request)
  : "${REQUEST_ID:?--request-id required}"
  IS_VIEW=1 invoke get_request --request-id "${REQUEST_ID}"
  ;;
get-stats)
  IS_VIEW=1 invoke get_stats
  ;;
list)
  if [ -n "${STATUS_FILTER}" ]; then
    IS_VIEW=1 invoke list_requests --status-filter "${STATUS_FILTER}" --offset "${OFFSET}" --limit "${LIMIT}"
  else
    IS_VIEW=1 invoke list_requests --offset "${OFFSET}" --limit "${LIMIT}"
  fi
  ;;
*)
  sed -n '2,20p' "$0"
  exit 1
  ;;
esac
