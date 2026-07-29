default: build

# soroban-sdk >= 22 requires the `wasm32v1-none` target. Rust 1.82+ enables
# reference-types/multi-value on `wasm32-unknown-unknown`, which the Soroban
# environment rejects, so building against the old target fails at build-script
# time regardless of contract code.
WASM_TARGET ?= wasm32v1-none
WASM := target/$(WASM_TARGET)/release/aegis_contracts.wasm

build:
	@command -v rustup >/dev/null 2>&1 \
		&& rustup target add $(WASM_TARGET) \
		|| echo "note: rustup not on PATH; assuming $(WASM_TARGET) is installed"
	cargo build --target $(WASM_TARGET) --release
	@echo "Build successful. WASM located at $(WASM)"

test:
	cargo test

# Off-chain monitoring service (real-time event streaming).
monitor-install:
	cd .github/monitoring && npm install

monitor-test:
	cd .github/monitoring && npm test

monitor:
	cd .github/monitoring && npm start

monitor-demo:
	cd .github/monitoring && npm run dev

# Regenerate the on-chain XDR fixtures consumed by monitoring's
# tests/onchain-compat.test.js.
dump-events:
	cargo test dump_event_xdr -- --ignored --nocapture

test-all: test monitor-test

fmt:
	cargo fmt --all

clean:
	cargo clean
	rm -rf .github/monitoring/node_modules .github/monitoring/data

optimize: build
	stellar contract optimize --wasm $(WASM)

.PHONY: default build test monitor-install monitor-test monitor monitor-demo dump-events test-all fmt clean optimize
