#!/usr/bin/env bash
# invoke-release.sh — see scripts/invoke.sh for full docs.
# Usage: ./scripts/invoke-release.sh --caller ... --request-id ... --source ...
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec "${ROOT}/scripts/invoke.sh" release "$@"
