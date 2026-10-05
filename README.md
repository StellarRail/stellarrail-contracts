# stellarrail-contracts

[![CI](https://github.com/StellarRail/stellarrail-contracts/actions/workflows/ci.yml/badge.svg)](https://github.com/StellarRail/stellarrail-contracts/actions/workflows/ci.yml)
[![Security](https://github.com/StellarRail/stellarrail-contracts/actions/workflows/security.yml/badge.svg)](https://github.com/StellarRail/stellarrail-contracts/actions/workflows/security.yml)

Soroban escrow contract for StellarRail: lock XLM (`deposit`), settle to a
destination (`release`), return to the funder (`refund`), rescue past-due
funds permissionlessly (`expire`) — plus admin roles, events, and upgrades.

> Interface is **frozen** at v1 (see `docs/adr/`): any change needs an ADR.
> Frozen entrypoints: `initialize`, `deposit`, `release`, `refund`, `expire`,
> `get_request`, `list_requests`, `get_stats`, `set_paused`,
> `transfer_admin`, `accept_admin`, `set_signer`, `version`, `migrate`.

## Interface

| Entrypoint | Auth | Effect |
|---|---|---|
| `initialize(admin, signer, token)` | `admin` signs, once | roles + SAC token + `Initialized` |
| `deposit(amount, request_id, depositor, destination?, deadline)` | `depositor` signs | pulls XLM, stores `Locked`, emits `FundsLocked` |
| `release(caller, request_id)` | admin OR `signer` | pays destination, `Released` + `PaymentReleased` |
| `refund(caller, request_id)` | admin OR `signer` | repays depositor, `Refunded` + `PaymentRefunded` |
| `expire(request_id)` | **anyone** (or no one) | repays depositor past deadline, `Expired` + `RequestExpired` |
| `get_request(request_id)` | none (view) | returns `EscrowRequest` |
| `list_requests(status?, offset, limit)` | none (view) | paged registry (cap 50) |
| `get_stats()` | none (view) | `{locked_count, locked_total, released_count, lifetime_volume}` |
| `set_paused(caller, bool)` | admin | emergency stop (`expire` unaffected) |
| `transfer_admin` / `accept_admin` | admin / staged admin | two-step rotation |
| `set_signer(caller, new)` | admin | immediate rotation |
| `version()` | none | `1` |
| `migrate(caller, hash)` | admin | code upgrade, state preserved |

`release`/`refund` take an explicit `caller` because Soroban `require_auth`
over fixed addresses expresses AND, not admin-OR-signer
(see `docs/adr/003-explicit-caller-param.md`).

Errors are typed codes (`docs/ERRORS.md`): `1 AlreadyExists`, `2 NotFound`,
`3 Unauthorized`, `4 InvalidAmount`, `5 InvalidDeadline`, `6 InvalidState`,
`7 Expired`, `8 NotExpired`, `9 Overflow`, `10 Paused`, `11 AlreadyInitialized`.

## Quickstart

```bash
make build        # stellar contract build → target/wasm32v1-none/release/escrow.wasm
make test         # cargo test --all-targets (unit + integration + fuzz)
make lint         # cargo clippy --all-targets -- -D warnings
SKIP_DEPLOY=1 make deploy-local   # build-only sandbox check
make deploy-local                 # deploy to local RPC if reachable
./scripts/deploy-testnet.sh       # testnet (needs SOURCE_SECRET/ADMIN_SECRET/…)
```

Full local loop: `docs/LOCAL_DEV.md`. Testnet: `docs/DEPLOY_TESTNET.md`.
Mainnet (gated pilot): `docs/DEPLOY_MAINNET.md`.

## Networks

| Network | Contract | WASM sha256 | Registry |
|---|---|---|---|
| testnet | `CDAV32AHVV6Q7FFNPUFA76AARTGWBE2QFIU3WAPDV64QIBRTGUFKLU2K` | `de7f457c…fbc1a` | `deployments/testnet.json` |
| mainnet | _pilot-pending_ | — | `deployments/mainnet.json` |

Live smoke evidence: `deployments/smoke-2026-10-04.json`
(1-XLM deposit→release + deposit→expire on testnet).

## Docs

`ARCHITECTURE.md` (module map, data flow, lifecycle) ·
`docs/{TOOLCHAIN,ERRORS,STORAGE,EVENTS,INDEXER,GAS,AUTH_MATRIX,AUDIT_CHECKLIST,UPGRADE,DEPLOY_TESTNET,DEPLOY_MAINNET,LOCAL_DEV,XLM_DESIGN}.md` ·
`docs/adr/` (design records) · `SECURITY.md`

## Audit status

Pre-audit self-assessment complete (`docs/AUDIT_CHECKLIST.md`, all ticked or
risk-accepted). External audit: _pending_ — do not mainnet without it.

## License

Dual MIT / Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`), per Soroban norms.
Dependency snapshot: `deployments/dependencies.txt` (`cargo audit` clean —
one transitive unmaintained-lint on `paste` via the pinned SDK's ark crypto,
no CVE, accepted).
