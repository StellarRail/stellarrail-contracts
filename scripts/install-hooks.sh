#!/usr/bin/env bash
# Installs the repo's git hooks (`.githooks/` -> `core.hooksPath`).
set -euo pipefail
cd "$(dirname "$0")/.."
git config core.hooksPath .githooks
chmod +x .githooks/pre-commit
echo "hooks installed: $(git config core.hooksPath)"
