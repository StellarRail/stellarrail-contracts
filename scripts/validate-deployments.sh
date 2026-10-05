#!/usr/bin/env bash
# Validate deployments/*.json against the registry schema.
#
# Schema per network file:
#   {network, contractId, wasmHash, admin, signer, deployedAt, txHash, version}
# builds.json:
#   {wasm, sha256, size, rustc, cargo, stellar, sorobanSdk, builtAt}
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FAIL=0

need() { # file jq-filter description
  local got
  got="$(jq -r "$2" "$1")"
  if [ "${got}" = "null" ] || [ -z "${got}" ]; then
    echo "INVALID ${1}: missing/empty $3" >&2
    FAIL=1
  fi
}

for net in testnet mainnet; do
  f="${ROOT}/deployments/${net}.json"
  [ -f "${f}" ] || { echo "INVALID: missing ${f}" >&2; FAIL=1; continue; }
  jq -e . "${f}" >/dev/null || { echo "INVALID: ${f} is not JSON" >&2; FAIL=1; continue; }
  [ "$(jq -r .network "${f}")" = "${net}" ] || { echo "INVALID ${f}: network mismatch" >&2; FAIL=1; }
  need "${f}" .version "version"
  if [ "${net}" = "testnet" ]; then
    for k in contractId wasmHash admin signer deployedAt txHash token; do
      need "${f}" ".${k}" "${k}"
    done
    # Strkey spot-checks.
    for k in contractId admin signer token; do
      v="$(jq -r ".${k}" "${f}")"
      case "${v}" in
      C* | G*) ;;
      *) echo "INVALID ${f}: ${k}=${v} not a strkey" >&2; FAIL=1 ;;
      esac
    done
  else
    echo "INFO ${f}: ${net} pilot-pending (template accepted)."
  fi
done

b="${ROOT}/deployments/builds.json"
[ -f "${b}" ] || { echo "INVALID: missing ${b}" >&2; FAIL=1; }
if [ -f "${b}" ]; then
  for k in wasm sha256 size rustc cargo stellar sorobanSdk builtAt; do
    need "${b}" ".${k}" "${k}"
  done
  # Hash must match the local build output.
  LOCAL="$(sha256sum "${ROOT}/target/wasm32v1-none/release/escrow.wasm" 2>/dev/null | cut -d' ' -f1 || echo MISSING)"
  RECORDED="$(jq -r .sha256 "${b}")"
  if [ "${LOCAL}" = "MISSING" ]; then
    echo "WARN: no local wasm to compare (run make build first)."
  elif [ "${LOCAL}" != "${RECORDED}" ]; then
    echo "INVALID builds.json: local ${LOCAL} != recorded ${RECORDED}" >&2
    FAIL=1
  fi
fi

if [ "${FAIL}" != "0" ]; then
  echo "deployments registry INVALID" >&2
  exit 1
fi
echo "deployments registry OK"
