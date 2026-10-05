# Reproducible CI builder for the escrow WASM.
# Usage: docker build -t escrow:local .   (or: make docker-build)
# The wasm is exported at /out/escrow.wasm + /out/escrow.wasm.sha256.

FROM rust:1.97-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Pinned stellar-cli binary (x86_64; compiling from source needs dbus dev
# libs and ~10 min — the binary is byte-identical in behavior, pinned hash).
RUN curl -sSL -o /tmp/stellar-cli.tgz https://github.com/stellar/stellar-cli/releases/download/v27.1.0/stellar-cli-27.1.0-x86_64-unknown-linux-gnu.tar.gz \
 && mkdir -p /tmp/stellar-cli && tar -xzf /tmp/stellar-cli.tgz -C /tmp/stellar-cli \
 && install "$(find /tmp/stellar-cli -name stellar -type f | head -1)" /usr/local/bin/stellar \
 && rm -rf /tmp/stellar-cli.tgz /tmp/stellar-cli \
 && stellar --version

RUN rustup target add wasm32v1-none

WORKDIR /work
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src

RUN stellar contract build \
 && mkdir -p /out \
 && cp target/wasm32v1-none/release/escrow.wasm /out/escrow.wasm \
 && sha256sum /out/escrow.wasm | cut -d' ' -f1 > /out/escrow.wasm.sha256

FROM alpine:3.21 AS export
COPY --from=builder /out/escrow.wasm /out/escrow.wasm.sha256 /out/
CMD ["sha256sum", "/out/escrow.wasm"]
