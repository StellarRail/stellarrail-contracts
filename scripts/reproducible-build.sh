#!/usr/bin/env bash
# Reproducible build: compile twice, require identical hashes, and record
# the artifact + toolchain in deployments/builds.json.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM="target/wasm32v1-none/release/escrow.wasm"

echo "[reproducible] build #1..."
stellar contract build >/dev/null
H1="$(sha256sum "${ROOT}/${WASM}" | cut -d' ' -f1)"
echo "[reproducible] build #2..."
touch "${ROOT}/src/lib.rs"
stellar contract build >/dev/null
H2="$(sha256sum "${ROOT}/${WASM}" | cut -d' ' -f1)"

if [ "${H1}" != "${H2}" ]; then
  echo "[reproducible] FAIL: consecutive builds differ (${H1} != ${H2})" >&2
  exit 1
fi
echo "[reproducible] stable hash: ${H1}"

SDK_VERSION="$(grep -A1 'name = "soroban-sdk"' "${ROOT}/Cargo.lock" | grep version | head -1 | cut -d'"' -f2)"
SIZE="$(wc -c <"${ROOT}/${WASM}")"
mkdir -p "${ROOT}/deployments"
jq -n \
  --arg wasm "${WASM}" \
  --arg sha256 "${H1}" \
  --argjson size "${SIZE}" \
  --arg rustc "$(rustc --version)" \
  --arg cargo "$(cargo --version)" \
  --arg stellar "$(stellar --version | head -1)" \
  --arg sdk "${SDK_VERSION}" \
  --arg builtAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  '{wasm:$wasm,sha256:$sha256,size:$size,rustc:$rustc,cargo:$cargo,
    stellar:$stellar,sorobanSdk:$sdk,builtAt:$builtAt}' \
  >"${ROOT}/deployments/builds.json"
echo "[reproducible] registry: deployments/builds.json"

echo "[reproducible] verify on another machine:"
echo "  1. check out the same commit + rust-toolchain.toml toolchain"
echo "  2. ./scripts/reproducible-build.sh"
echo "  3. compare deployments/builds.json sha256"
