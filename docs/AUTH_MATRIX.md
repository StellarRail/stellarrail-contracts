# Access-control matrix

Privileged entrypoints authenticate an explicit `caller`
(`caller.require_auth()` + role comparison) so denials surface typed
`Unauthorized` instead of a host auth trap. `deposit` authorizes the
`depositor` (any funder is accepted); `expire` and all views need no auth.

## Matrix (proven by `test::auth_matrix` — 12 combos)

| Caller \ Entrypoint | `release` | `refund` | `expire` (past deadline) | `set_paused` |
|---|---|---|---|---|
| admin | ✅ settles | ✅ settles | ✅ (caller ignored) | ✅ toggles |
| signer | ✅ settles | ✅ settles | ✅ (caller ignored) | ❌ `Unauthorized` |
| stranger (authed, wrong role) | ❌ `Unauthorized` | ❌ `Unauthorized` | ✅ (caller ignored) | ❌ `Unauthorized` |
| unauthenticated | 🪤 host auth trap | 🪤 host auth trap | ✅ (caller ignored) | 🪤 host auth trap |

Legend: ✅ allowed · ❌ typed `EscrowError::Unauthorized` (maps to API 403) ·
🪤 Soroban host rejects the missing auth before contract code runs
(surfaces as `Err(Err(_))` on `try_` clients — never a silent success).

## Notes

- **Why an explicit `caller` parameter?** Soroban `require_auth` over fixed
  addresses expresses AND, not the admin-OR-signer rule. Taking `caller` and
  comparing against both stored roles is the standard OR pattern; see
  `docs/adr/003-explicit-caller-param.md`.
- **`expire` ignores identity by design:** it can only ever pay the original
  depositor, so there is nothing to steal — anyone (keeper bots included)
  may rescue past-due funds, even while paused or with no auth configured.
- **`deposit` accepts any authenticated funder:** allowlisting depositors is
  an API-layer concern; on-chain, `depositor.require_auth()` plus amount /
  deadline / uniqueness gates are sufficient.
- **Two-step rotation** (`transfer_admin` → `accept_admin`) keeps a stolen
  admin key from instantly handing power to an attacker mid-flight: the
  staged address must separately authorize acceptance.
