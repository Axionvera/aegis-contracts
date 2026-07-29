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
	cd monitoring && npm install

monitor-test:
	cd monitoring && npm test

monitor:
	cd monitoring && npm start

monitor-demo:
	cd monitoring && npm run dev

# Regenerate the on-chain XDR fixtures consumed by monitoring's
# tests/onchain-compat.test.js.
dump-events:
	cargo test dump_event_xdr -- --ignored --nocapture

test-all: test monitor-test

# Automated portion of docs/release-checklist.md: storage, event, dashboard and
# spec-documentation compatibility between the contract and its consumers.
compat-check:
	@bash ./scripts/check-release-compat.sh

# Full pre-PR gate: compatibility, formatting, lints, contract tests, WASM
# build, and the off-chain monitoring test suite.
verify: compat-check
	cargo fmt --all --check
	cargo clippy --all-targets -- -D warnings
	cargo test
	$(MAKE) build
	cd monitoring && npm test

# Release gate. Runs everything in `verify`, then prints the manual checklist
# sections that still require human sign-off.
release-check: verify
	@echo ""
	@echo "==================================================================="
	@echo " Automated release checks PASSED."
	@echo ""
	@echo " Now complete the manual sections of docs/release-checklist.md:"
	@echo "   2. Storage & state compatibility (TTL/archival, migrations)"
	@echo "   4. Errors and panics"
	@echo "   5. Roles and access control (see standing risks R1-R3)"
	@echo "   6. Compliance enforcement"
	@echo "   7. SDK and client compatibility"
	@echo "   9. Security and audit"
	@echo ""
	@echo " WASM sha256:"
	@sha256sum $(WASM) 2>/dev/null || echo "   (run 'make build' first)"
	@echo ""
	@echo " Then copy the Sign-Off Record from docs/release-checklist.md"
	@echo " into the release PR or tag notes."
	@echo "==================================================================="

fmt:
	cargo fmt --all

clean:
	cargo clean
	rm -rf monitoring/node_modules monitoring/data

optimize: build
	stellar contract optimize --wasm $(WASM)

.PHONY: default build test verify compat-check release-check monitor-install monitor-test monitor monitor-demo dump-events test-all fmt clean optimize
