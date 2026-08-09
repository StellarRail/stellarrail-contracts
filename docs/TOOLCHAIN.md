# Toolchain

Reproducible build pins for `stellarrail-contracts`. A fresh machine follows
these steps to reach green `make test`.

## Pinned versions

| Tool | Pinned version | Source |
|---|---|---|
| Rust | `1.97.1` | `rust-toolchain.toml` (`channel = "1.97.1"`) |
| soroban-sdk | `28.0.0` | `Cargo.toml` + `Cargo.lock` (`soroban-env-host 28.0.2`) |
| stellar-cli | `27.1.0` | install docs below |
| Node (JS clients/scripts) | `20` | `.nvmrc` |
| wasm target | `wasm32v1-none` | `rust-toolchain.toml` targets (SDK ≥ 22 requires `wasm32v1-none`; `wasm32-unknown-unknown` is rejected by the SDK build script on rust ≥ 1.82) |

Recorded:

```text
rustc 1.97.1 (8bab26f4f 2026-07-14)
cargo 1.97.1 (c980f4866 2026-06-30)
stellar 27.1.0 (8e402ea28202950b272fbabc34caad4d2f64fe87)
node v20.20.2
soroban-sdk 28.0.0 (lockfile: soroban-env-host 28.0.2)
```

> Note: `build.md` suggested SDK 21.x. SDK 21/22 host crates do not compile
> on rust ≥ 1.82-era toolchains (broken `ed25519-dalek`/`rand` bounds in
> `soroban-env-host`), and SDK ≥ 22 build scripts reject
> `wasm32-unknown-unknown` on rust ≥ 1.82. SDK 28 + `wasm32v1-none` is the
> minimal combination green on this toolchain. The contract code uses only
> long-stable SDK APIs.

## Fresh-machine setup

```bash
# 1. Rust (pinned automatically by rust-toolchain.toml via rustup)
rustup show active-toolchain

# 2. WASM target (also auto-installed by rustup from rust-toolchain.toml)
rustup target list --installed

# 3. Stellar CLI (provides `stellar contract build/invoke/deploy`)
cargo install stellar-cli --locked
stellar --version   # expect 27.1.0

# 4. Node for JS clients (optional)
nvm use             # reads .nvmrc

# 5. Build + test
make build
make test
```

## Make targets

| Target | Command |
|---|---|
| `make build` | `stellar contract build` |
| `make test` | `cargo test --all-targets` |
| `make fmt` | `cargo fmt --all` |
| `make lint` | `cargo clippy --all-targets -- -D warnings` |
| `make deploy-local` | `./scripts/sandbox.sh` |
| `make deploy-testnet` | `./scripts/deploy-testnet.sh` |
| `make report-gas` | wasm size + sha256 |
| `make reproducible` | `./scripts/reproducible-build.sh` |
| `make docker-build` | `docker build -t escrow:local .` |
