.PHONY: build test fmt lint deploy-local deploy-testnet report-gas reproducible docker-build clean

build:
	stellar contract build

test:
	cargo test --all-targets

fmt:
	cargo fmt --all

lint:
	cargo clippy --all-targets -- -D warnings

deploy-local:
	./scripts/sandbox.sh

deploy-testnet:
	./scripts/deploy-testnet.sh

report-gas:
	@echo "per-entrypoint footprint report"
	@ls -lh target/wasm32v1-none/release/escrow.wasm 2>/dev/null || ls -lh target/wasm32*/release/*.wasm
	@sha256sum target/wasm32v1-none/release/escrow.wasm 2>/dev/null || sha256sum target/wasm32*/release/*.wasm

reproducible:
	./scripts/reproducible-build.sh

docker-build:
	docker build -t escrow:local .

clean:
	cargo clean
	rm -rf test_snapshots
