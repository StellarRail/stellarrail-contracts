# Error codes

Every contract failure surfaces a typed `EscrowError` discriminant
(`src/errors.rs`). The chain service / API maps them to HTTP as below.
The contract never uses bare `panic!` outside tests.

| Code | `EscrowError` | Trigger | Suggested API mapping |
|---|---|---|---|
| 1 | `AlreadyExists` | `deposit` reuses a `request_id`; second `initialize` | `409 Conflict` |
| 2 | `NotFound` | unknown `request_id` in any entrypoint | `404 Not Found` |
| 3 | `Unauthorized` | caller is not admin/signer (or not admin for admin ops) | `403 Forbidden` |
| 4 | `InvalidAmount` | `amount <= 0` or `> MAX_AMOUNT` (50M XLM, stroops) | `400 Bad Request` |
| 5 | `InvalidDeadline` | `deadline` outside `(now + 60s, created + 30d]` | `400 Bad Request` |
| 6 | `InvalidState` | op not allowed from current status (e.g. double release) | `409 Conflict` |
| 7 | `Expired` | `release` at/after `deadline` (call `expire` instead) | `410 Gone` |
| 8 | `NotExpired` | `expire` at/before `deadline` | `409 Conflict` |
| 9 | `Overflow` | checked-arithmetic overflow (amounts, counters) | `500 Internal` (alert) |
| 10 | `Paused` | mutating op while paused (`expire` still allowed) | `503 Unavailable` |
| 11 | `AlreadyInitialized` | second `initialize` | `409 Conflict` |

## Reading the code from a simulation

`stellar transaction simulate` (or RPC `simulateTransaction`) returns the
`ContractError` code in the error string, e.g.
`error: EscrowError(3)` → `Unauthorized` → HTTP 403. The JS client
(`clients/js/`) maps these automatically — see `errorMapping.ts`.

## Panic policy

Contract (non-test) code **never panics**: every fallible path returns
`Result<_, EscrowError>`, there is no `panic!`/`unwrap()`/`expect()` outside
`#[cfg(test)]` modules, and arithmetic is checked (`checked_*` → `Overflow`).
A `panic!` in this repo therefore always means a broken test harness, never
a user-facing failure mode. Audit with:

```bash
grep -rn --include='*.rs' -e 'panic!\|panic_with_error\|\.unwrap()\|\.expect(' src/
# Every hit must sit inside a #[cfg(test)] module (verified: 13 hits, all in
# src/lib.rs / src/events.rs test helpers + page assertions).
```
