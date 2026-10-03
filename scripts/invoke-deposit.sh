#!/usr/bin/env bash
# invoke-deposit.sh — see scripts/invoke.sh for full docs.
# Usage: ./scripts/invoke-deposit.sh --amount ... --request-id ... --depositor ... --destination ... --deadline ... --source ...
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec "${ROOT}/scripts/invoke.sh" deposit "$@"
