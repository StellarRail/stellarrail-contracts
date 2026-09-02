# Upgrade guide

The escrow contract is upgradeable by admin action. Upgrades replace code,
never state.

## How it works

- `migrate(caller, new_wasm_hash)` (admin only) calls
  `deployer.update_current_contract(Wasm(hash))`.
- All state lives in **persistent** storage under `DataKey` (see
  `docs/STORAGE.md`), which survives code replacement untouched.
- `version()` returns `SCHEMA_VERSION`. The release workflow
  (`.github/workflows/release.yml`) refuses to tag a release whose tag
  disagrees with the on-chain `version()`.

## State-compat checklist (every upgrade)

- [ ] No `DataKey` variant removed or reordered (`Request`, `Admin`,
      `Signer`, `Token`, `Paused`, `AllIds`, `Stats`, `PendingAdmin`).
- [ ] No field removed/renamed in `EscrowRequest`, `EscrowStatus`,
      or `Stats` (append-only changes; bump `SCHEMA_VERSION`).
- [ ] `EscrowError` discriminants unchanged (API mapping depends on them).
- [ ] Event topics/payloads unchanged, or the indexer is updated in lockstep.
- [ ] Testnet drill (below) green, with pre-existing `Locked` requests
      still listed, releasable, refundable, and expirable afterwards.

## Testnet drill

```bash
# 1. Deploy v1, initialize, create 2 locked requests (see docs/DEPLOY_TESTNET.md).
# 2. Build the candidate WASM and upload it:
stellar contract upload --wasm target/wasm32v1-none/release/escrow.wasm \
  --source ADMIN --network testnet
#    -> prints NEW_WASM_HASH
# 3. Migrate:
stellar contract invoke --id <CONTRACT> --source ADMIN --network testnet \
  -- migrate --caller <ADMIN> --new-wasm-hash <NEW_WASM_HASH>
# 4. Verify state preserved:
stellar contract invoke --id <CONTRACT> --source ADMIN --network testnet \
  -- version            # -> 1 (or the bumped version)
./scripts/invoke.sh get-stats
./scripts/invoke.sh list --offset 0 --limit 50
# 5. Settle one pre-existing request to prove code paths work post-upgrade.
```

Record the drill (hashes, before/after `get_stats`) in
`deployments/upgrade-drill.md` (ISSUE-050).
