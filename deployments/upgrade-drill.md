# Upgrade drill (testnet, v1 → v1) — 2026-10-04

Contract: `CDAV32AHVV6Q7FFNPUFA76AARTGWBE2QFIU3WAPDV64QIBRTGUFKLU2K`
([explorer](https://stellar.expert/explorer/testnet/contract/CDAV32AHVV6Q7FFNPUFA76AARTGWBE2QFIU3WAPDV64QIBRTGUFKLU2K))

## What was proven

A live contract holding unsettled escrows was migrated to a rebuilt WASM
(same source → same hash, i.e. a v1→v1 mock upgrade) with **zero state
loss**, and settlement paths worked after the upgrade.

## Log

1. Created 2 fresh locked requests (1 XLM each):
   - deposit A tx `e43010cdce4edb2b648b1d0de49924057e2634f73d2a0d6b5485b080caf87c7e`
     (`a87f997a…a9a7afe8c`)
   - deposit B tx `d5da02223d7e80ff97db7b5d1f4182be791eeac5b0ccc7718c99df9f85ebe8e9`
     (`b928e7e9…ae1fe41`)
2. Baseline `get_stats`: `{locked_count: 4, locked_total: 40000000,
   released_count: 3, lifetime_volume: 110000000}`.
3. Rebuilt WASM locally: `sha256 =
   de7f457c643389fcf07be011391f75a26ffc4679d406771219f1b49e394fbc1a`
   — **bit-identical to the deployed hash** (reproducibility evidence).
4. Uploaded (already installed — skipped) + `migrate`:
   tx `8f2dd21ccc60e62df696e9c523a0afc2a56a1adc9c7ff84f66aab9bd06e7c935`.
5. Post-migrate checks:
   - `version()` → `1`.
   - `get_stats` → identical to baseline (all counters preserved).
   - `get_request(A)` → intact `Locked` record.
6. Post-upgrade settlement:
   - `release(A)` tx `f65a4d0e5dbd18d723b93dafcd55c09c935fb85bbe533c319289ea35992a713e`
     → `PaymentReleased`.
   - `refund(B)` (by signer) tx
     `99db59c1900f8ecc0a1e0692878e8121890599e67b64e9643d0010181bbd9f58`
     → `PaymentRefunded`.
   - Final `get_stats`: `{locked_count: 2, locked_total: 20000000,
     released_count: 4, lifetime_volume: 110000000}` (2 older locked
     escrows remain; all transitions accounted).

## Conclusion

✅ Upgrade preserves requests + counters; all entrypoints functional after
`migrate`. The drill procedure is scripted in `docs/UPGRADE.md`.
