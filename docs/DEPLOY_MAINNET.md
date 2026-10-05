# Mainnet deployment guardrails (RC1)

Mainnet is a gated ceremony, never a script run. Status: **pilot-pending**
(no mainnet deployment exists yet).

## Gates (all must hold)

1. **Audit + freeze**: `docs/AUDIT_CHECKLIST.md` ticked, interface frozen
   (`docs/adr/`), tag `contracts-v1.0.0-rc1` cut.
2. **Testnet proof**: `deployments/testnet.json` + smoke evidence
   (`deployments/smoke-*.json`) green, upgrade drill recorded
   (`deployments/upgrade-drill.md`).
3. **Reproducible WASM**: local `sha256` matches `deployments/builds.json`;
   `scripts/deploy-mainnet.sh` aborts on drift.
4. **Multisig admin**: the `ADMIN_ADDRESS` is an N-of-M multisig; the
   `SIGNER_ADDRESS` is HSM-backed. No raw keys on workstations.
5. **Monitoring**: event poller (`docs/INDEXER.md`) watches `Paused`,
   `PaymentReleased`, `RequestExpired` before the first real deposit.

## Ceremony

```bash
# 1. Dry-run everything on testnet one final time.
# 2. Re-verify the WASM hash out-of-band (second engineer, second machine).
# 3. Deploy (refuses without the exact confirmation string + 10s countdown):
export CONFIRM_MAINNET=DEPLOY-TO-MAINNET SOURCE_SECRET=... \
  ADMIN_ADDRESS=... SIGNER_ADDRESS=... TOKEN_ADDRESS=... RPC_URL=...
./scripts/deploy-mainnet.sh
# 4. Initialize via the multisig ceremony (script does NOT auto-initialize).
# 5. Pilot: ONE small-value escrow (≤ 10 XLM) end-to-end, then pause 24h.
# 6. Record everything in deployments/mainnet.json.
```

## Rollback

- Bugs in code → fix → rebuild → `migrate` (admin/multisig) per
  `docs/UPGRADE.md`; state is preserved.
- Active exploit → `set_paused(true)` immediately (pause drill contacts in
  `SECURITY.md`); `expire` keeps rescuing funds while paused.
- Worst case → see `docs/DISASTER_RECOVERY.md` (forthcoming, ISSUE-065;
  direct CLI settlement without the API).

## Pilot limits

- Week 1: ≤ 10 XLM per escrow, ≤ 100 XLM aggregate locked.
- Limits lift only after 7 clean days + event/CCTV review of every
  settlement leg.
