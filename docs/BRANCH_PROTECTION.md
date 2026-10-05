# Branch protection (for the repo admin)

Protect `main` with these required status checks (Settings → Branches →
Add rule):

- `fmt`
- `clippy`
- `test`
- `build`
- `security` (from `security.yml`)
- Require branches to be up to date before merging: **on**
- Require pull request reviews before merging: **1 approval**
- Dismiss stale approvals on new commits: **on**
- `testnet` (smoke) and `release` workflows are `workflow_dispatch`-only
  and must NOT be required checks.

Rationale: every merge to `main` proves fmt + clippy + full test suite +
reproducible WASM build + supply-chain audit. Live-network workflows never
run on PRs.
