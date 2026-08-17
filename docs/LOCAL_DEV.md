# Local development

Fast inner loop: unit tests first, sandbox second, testnet third.

## 1. Unit tests (default)

```bash
make test        # cargo test --all-targets
make lint        # clippy -D warnings
```

## 2. Local sandbox

```bash
make deploy-local            # build + deploy to local RPC if reachable
SKIP_DEPLOY=1 make deploy-local   # build only (CI-safe)
```

`scripts/sandbox.sh` behavior:

1. `stellar contract build` → `target/wasm32v1-none/release/escrow.wasm`.
2. If `$RPC_URL` (default `http://localhost:8000/rpc`) answers `getHealth`,
   deploys the WASM and writes the id to `.sandbox-contract-id`.
3. Otherwise prints how to start a network and exits 0 (build still verified).

Start a local network with docker:

```bash
stellar network start --limits unlimited
```

Then initialize (after ISSUE-011) and exercise the contract:

```bash
export CONTRACT_ID="$(cat .sandbox-contract-id)"
./scripts/invoke.sh deposit --amount 10000000 --request-id <64hex> \
  --depositor <G...> --destination <G...> --deadline <unix> --source admin
./scripts/invoke.sh release --caller <G...> --request-id <64hex> --source admin
./scripts/invoke.sh get-request --request-id <64hex>
./scripts/invoke.sh get-stats
```

Full local deposit→release loop is covered automatically by
`tests/integration_sandbox.rs` (ISSUE-047) without needing docker.
