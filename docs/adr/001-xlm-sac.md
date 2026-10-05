# ADR-001: XLM via the Stellar Asset Contract (SAC)

- Status: accepted
- Date: 2026-10-04
- Context: ISSUE-010 design

## Problem

v1 settles native XLM. Contracts cannot move native balances directly; the
only on-chain interface is the SAC token contract, whose id differs per
network.

## Decision

- Store one `Token` address at `initialize`; move funds exclusively through
  `token::Client::transfer` (pull on `deposit`, push on settlement).
- The contract never mints and never wraps: pure pass-through vault.
- Resolve the SAC id per network at deploy time via
  `stellar contract id asset --asset native --network <net>`; record it in
  `deployments/<network>.json`. Verified testnet id:
  `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC`.
- No custom assets in v1 (PRD non-goal).

## Consequences

- Multi-asset later = one `token` field per request + allowlist; call sites
  already take the token address as a parameter, so the change is additive.
- Tests mint via `env.register_stellar_asset_contract_v2`.
