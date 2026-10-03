#!/usr/bin/env bash
# invoke-expire.sh — see scripts/invoke.sh for full docs.
# Usage: ./scripts/invoke-expire.sh --request-id ... --source ...
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec "${ROOT}/scripts/invoke.sh" expire "$@"
