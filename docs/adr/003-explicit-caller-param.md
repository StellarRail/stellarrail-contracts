# ADR-003: Explicit `caller` parameter on privileged entrypoints

- Status: accepted
- Date: 2026-10-04
- Context: ISSUE-011/014/016 design

## Problem

`release`/`refund` must be callable by the admin **or** the signer, and
denials must surface the typed `Unauthorized` code (API maps it to 403).
Soroban's `require_auth(address)` passes only if that address authorized the
call: calling it on two fixed addresses expresses AND (both must sign), and
a missing auth traps at the host (no typed code, indistinguishable from
misconfiguration on the API side).

## Decision

Privileged entrypoints take an explicit `caller: Address`:

- `caller.require_auth()` proves liveness of the caller,
- `caller == admin || caller == signer` (else `Unauthorized`) proves the role.

So `release(caller, request_id)`, `refund(caller, request_id)`,
`set_paused/transfer_admin/set_signer/migrate(caller, …)`. `expire` and
views take no caller (permissionless / read-only by design).

## Consequences

- The interface differs from the earliest sketch (which had bare
  `release(request_id)`): the sketch could not express OR-of-two-roles.
- Tests can assert the *role* check independently of auth (exact mocks),
  giving typed denials instead of host traps (`test::auth_matrix`).
- CLI/API callers pass `--caller` explicitly; indexers record it as the
  event `actor`.
