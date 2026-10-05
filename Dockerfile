# Reproducible CI builder for the escrow WASM.
# Usage: docker build -t escrow:local .   (or: make docker-build)
# The wasm is exported at /out/escrow.wasm + /out/escrow.wasm.sha256.

FROM rust:1.97-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32v1-none \
 && cargo install stellar-cli --locked --version 27.1.0

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
