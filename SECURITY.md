# Security: threat model + invariants

## Threat model

| Actor | Capability | Contract defense |
|---|---|---|
| Malicious operator (stolen admin key) | pause, rotate signer, migrate, settle any escrow | Two-step rotation limits instant takeover; pause is visible on-chain (`Paused` event); migration is the explicit upgrade path (see `docs/UPGRADE.md`). Key custody (multisig/HSM) is an ops requirement — see `docs/PRODUCTION.md` (forthcoming, ISSUE-066). |
| Rogue approver (stolen signer key) | release/refund any escrow | Admin rotates the signer in one tx (`set_signer`); old signer invalid immediately. Signer cannot pause, rotate admin, or migrate. |
| Random stranger | call any permissionless path | Only `expire` + views are open, and `expire` can solely pay the original depositor. Everything else denies typed `Unauthorized`. |
| Compromised / fake token | reenter or lie about balances | The contract talks to exactly one SAC id (stored at `initialize`); the Soroban host forbids reentrancy; status is written before the external transfer. |
| Chain reorg / RPC lag | deadline-edge confusion | Deadlines use ledger close time; API applies 300s grace (`docs/STORAGE.md`). Terminal states never transition, so replays are no-ops that fail `InvalidState`. |
| Storage eviction | dormant request record disappears | 30d TTL ≥ max deadline; keepers expire promptly; upgrade recovery documented (`docs/STORAGE.md` §Eviction). |

Out of scope for the contract (handled by API/ops layers): depositor
allowlisting, FX/quote logic, fiat settlement, front-end spoofing.

## Invariants (each maps to a test)

1. **Locked supply == sum of `Locked` amounts.** The contract's SAC balance
   equals `locked_total` whenever no settlement is in flight.
   → `release_transfer_event_status`, `refund_transfer_event_status`
   (contract balance returns to 0 per request).
2. **Terminal states are final.** `Released`/`Refunded`/`Expired`/`Failed`
   never transition; every second mutation fails `InvalidState` with
   balances unchanged.
   → `edge_double_ops_move_no_funds`, `refund_guards_after_release`,
   `expire_after_release_fails`.
3. **Only admin/signer move funds.** `release`/`refund` from anyone else fail
   `Unauthorized`; unauthenticated calls trap at the host.
   → `test::auth_matrix`, `test::admin_authz_denies_strangers`.
4. **Past-due funds are always rescuable.** `expire` works permissionless,
   including while paused, and pays only the depositor.
   → `expire_by_third_party_refunds_depositor`,
   `pause_blocks_mutations_but_not_expire`.
5. **No silent accounting drift.** Counter updates are checked; overflow
   fails the whole tx atomically (request + counters + events all roll back).
   → `amounts_overflow_guarded`, `overflow_release_underflow_guarded`,
   `reentrancy_failed_transfer_rolls_back`.
6. **Every mutation emits exactly one typed event; failures emit none.**
   → `events_suite_*`, `events_suite_failed_calls_emit_nothing`.

## Disclosure

Found a vulnerability? Email **security@stellarrail.example** (placeholder —
replace before mainnet) with: affected version (`version()` output), network
+ contract id, reproduction steps or PoC, and impact assessment.

- Acknowledgement within 2 business days; triage within 5.
- Coordinated disclosure: 90 days to fix before public disclosure.
- Severity guide: loss/misdirection of locked funds = critical; griefing
  (e.g. TTL edge strandings) = medium; docs/gas = low.
- Safe harbor: good-faith research on testnet is welcome; do not touch
  mainnet funds, do not exfiltrate data, do not degrade service.

## Supported versions

| Contract version | Status |
|---|---|
| `contracts-v1.0.0-rc1` (tag cut at ISSUE-070) | supported: testnet pilot only until mainnet pilot criteria pass (`docs/DEPLOY_MAINNET.md`) |
| pre-release / untagged builds | unsupported — do not use with real funds |

External audit status: self-assessment complete (`docs/AUDIT_CHECKLIST.md`);
independent audit **pending** — mainnet pilot requires it
(see `docs/PRODUCTION.md`, forthcoming ISSUE-066).

## Pause authority

The admin key (multisig at mainnet — see `docs/PRODUCTION.md`, forthcoming
ISSUE-066) may invoke
`set_paused` at any time. Pause drill contacts (template):

- Primary on-call: _name / phone / stellar address_
- Secondary: _name / phone / stellar address_
- Multisig signers: _2-of-3 addresses_

Pause drill: freeze → announce → diagnose → fix-or-migrate → unpause, with
`RequestExpired`/`PaymentReleased` event monitoring throughout (see
`docs/DISASTER_RECOVERY.md`, forthcoming ISSUE-065).
