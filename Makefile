# ─── Aegis RWA Contracts — Makefile ───────────────────────────────────────────
#
# Every target below is documented in docs/local-deployment.md.
# Run `make help` for a self-documenting list of targets.
#
# Configuration is driven by environment variables, all of which can be set in
# a local `.env` file (copy `.env.example` -> `.env`). See docs/local-deployment.md
# for the full reference of each variable.
# ──────────────────────────────────────────────────────────────────────────────

# Load .env if present so `make deploy` etc. pick up local configuration.
# `-include` does not fail when the file is missing.
ifneq (,$(wildcard ./.env))
-include .env
export
endif

# ─── Configuration (override on the CLI: `make build WASM_TARGET=...`) ────────

# Soroban SDK >= 22 requires the `wasm32v1-none` target on Rust 1.82+.
# The legacy `wasm32-unknown-unknown` target enables reference-types/multi-value,
# which the Soroban environment rejects at build time.
WASM_TARGET      ?= wasm32v1-none
CONTRACT_NAME    ?= aegis_contracts
PROFILE          ?= release

# `stellar` is the current CLI binary. The old `soroban` binary is still
# accepted for contributors who have not migrated yet.
STELLAR          ?= stellar

# Network / identity used by the deploy + invoke targets.
NETWORK          ?= local
SOURCE_ACCOUNT   ?= aegis-admin
RPC_URL          ?= http://localhost:8000/rpc
NETWORK_PASSPHRASE ?= Standalone Network ; February 2017

# Populated after a deploy; used by the invoke helper targets.
CONTRACT_ID      ?=
ADMIN_ADDRESS    ?=

WASM_DIR         := target/$(WASM_TARGET)/$(PROFILE)
WASM             := $(WASM_DIR)/$(CONTRACT_NAME).wasm
WASM_OPTIMIZED   := $(WASM_DIR)/$(CONTRACT_NAME).optimized.wasm
DEPLOY_OUT       := .aegis-deploy

.DEFAULT_GOAL := help
.PHONY: help build build-legacy test test-verbose fmt fmt-check clippy clean \
        optimize check-cli check-target deploy initialize invoke-status \
        network-up network-down network-add fund verify verify-interface all ci

# ─── Help ─────────────────────────────────────────────────────────────────────

## help: Show this help message (default target).
help:
	@echo "Aegis RWA Contracts — available make targets"
	@echo ""
	@grep -E '^## [a-zA-Z_-]+:' $(MAKEFILE_LIST) \
		| sed -e 's/^## //' \
		| awk '{ i = index($$0, ": "); \
		         printf "  \033[36m%-18s\033[0m %s\n", substr($$0, 1, i - 1), substr($$0, i + 2) }'
	@echo ""
	@echo "Configuration (current values):"
	@echo "  WASM_TARGET        = $(WASM_TARGET)"
	@echo "  NETWORK            = $(NETWORK)"
	@echo "  SOURCE_ACCOUNT     = $(SOURCE_ACCOUNT)"
	@echo "  RPC_URL            = $(RPC_URL)"
	@echo "  CONTRACT_ID        = $(if $(CONTRACT_ID),$(CONTRACT_ID),<unset — run 'make deploy'>)"
	@echo ""
	@echo "Full guide: docs/local-deployment.md"

# ─── Build ────────────────────────────────────────────────────────────────────

## check-target: Verify the required Rust wasm target is installed.
check-target:
	@rustup target list --installed 2>/dev/null | grep -qx "$(WASM_TARGET)" || { \
		echo "ERROR: Rust target '$(WASM_TARGET)' is not installed."; \
		echo "Fix:   rustup target add $(WASM_TARGET)"; \
		exit 1; }

## build: Compile the contract to WASM (target set by WASM_TARGET).
build: check-target
	cargo build --target $(WASM_TARGET) --release
	@echo "Build successful. WASM at $(WASM)"

## build-legacy: Build for wasm32-unknown-unknown (requires Rust <= 1.81 ONLY).
build-legacy:
	@echo "WARNING: 'wasm32-unknown-unknown' is only supported on Rust 1.81 and"
	@echo "         earlier. On newer toolchains soroban-sdk aborts the build."
	@echo "         Prefer 'make build' (target wasm32v1-none)."
	cargo build --target wasm32-unknown-unknown --release

## optimize: Build then shrink the WASM with the Stellar CLI.
optimize: check-cli build
	$(STELLAR) contract optimize --wasm $(WASM)
	@echo "Optimized WASM at $(WASM_OPTIMIZED)"

# ─── Test / quality ───────────────────────────────────────────────────────────

## test: Run the full unit test suite (native target, no wasm needed).
test:
	cargo test

## test-verbose: Run tests with captured stdout shown.
test-verbose:
	cargo test -- --nocapture

## fmt: Format all Rust sources in place.
fmt:
	cargo fmt --all

## fmt-check: Fail if any source file is not correctly formatted.
fmt-check:
	cargo fmt --all -- --check

## clippy: Run the linter and treat warnings as errors.
clippy:
	cargo clippy --all-targets -- -D warnings

## ci: Run the checks CI enforces (fmt-check + clippy + test + build).
ci: fmt-check clippy test build

## verify: Single local gate contributors should run before pushing.
#   Runs formatting check, lint, the test suite, and a release build in one
#   command. This is the recommended pre-push check to avoid failing CI.
#   (clippy is skipped if the toolchain lacks it; build/test still run.)
verify: fmt-check clippy test build
	@echo ""
	@echo "Local verification passed: fmt-check + clippy + test + build."

## all: Format, test, and build.
all: fmt test build

## clean: Remove build artifacts and local deploy state.
clean:
	cargo clean
	@rm -f $(DEPLOY_OUT)

# ─── Local network ────────────────────────────────────────────────────────────

## check-cli: Verify the Stellar CLI is installed and on PATH.
check-cli:
	@command -v $(STELLAR) >/dev/null 2>&1 || { \
		echo "ERROR: '$(STELLAR)' not found on PATH."; \
		echo "Fix:   cargo install --locked stellar-cli"; \
		echo "       (or set STELLAR=soroban if you use the legacy binary)"; \
		exit 1; }

## network-up: Start a local Stellar network in Docker (quickstart image).
network-up: check-cli
	$(STELLAR) container start local
	@echo "Local network starting. Health: curl $(RPC_URL) -d '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getHealth\"}' -H 'Content-Type: application/json'"

## network-down: Stop the local Stellar network container.
network-down: check-cli
	$(STELLAR) container stop local

## network-add: Register the 'local' network in the Stellar CLI config.
network-add: check-cli
	$(STELLAR) network add $(NETWORK) \
		--rpc-url "$(RPC_URL)" \
		--network-passphrase "$(NETWORK_PASSPHRASE)"

## fund: Create (if needed) and fund SOURCE_ACCOUNT on NETWORK.
fund: check-cli
	@$(STELLAR) keys address $(SOURCE_ACCOUNT) >/dev/null 2>&1 \
		|| $(STELLAR) keys generate --global $(SOURCE_ACCOUNT) --network $(NETWORK)
	$(STELLAR) keys fund $(SOURCE_ACCOUNT) --network $(NETWORK)
	@echo "Funded: $$($(STELLAR) keys address $(SOURCE_ACCOUNT))"

# ─── Deploy / invoke ──────────────────────────────────────────────────────────

## deploy: Deploy the built WASM to NETWORK; writes the id to .aegis-deploy.
deploy: check-cli build
	@$(STELLAR) contract deploy \
		--wasm $(WASM) \
		--source-account $(SOURCE_ACCOUNT) \
		--network $(NETWORK) \
		| tee $(DEPLOY_OUT)
	@echo ""
	@echo "Contract deployed. Id saved to $(DEPLOY_OUT)."
	@echo "Next: make initialize CONTRACT_ID=$$(cat $(DEPLOY_OUT))"

## initialize: Call initialize(admin) on the deployed contract.
initialize: check-cli
	@test -n "$(CONTRACT_ID)" || { \
		echo "ERROR: CONTRACT_ID is not set."; \
		echo "Fix:   make initialize CONTRACT_ID=\$$(cat $(DEPLOY_OUT))"; exit 1; }
	@test -n "$(ADMIN_ADDRESS)" || { \
		echo "ERROR: ADMIN_ADDRESS is not set."; \
		echo "Fix:   make initialize CONTRACT_ID=... ADMIN_ADDRESS=\$$($(STELLAR) keys address $(SOURCE_ACCOUNT))"; exit 1; }
	$(STELLAR) contract invoke \
		--id $(CONTRACT_ID) \
		--source-account $(SOURCE_ACCOUNT) \
		--network $(NETWORK) \
		-- initialize --admin $(ADMIN_ADDRESS)

## invoke-status: Read back core state (supply, pause, asset status).
invoke-status: check-cli
	@test -n "$(CONTRACT_ID)" || { echo "ERROR: CONTRACT_ID is not set."; exit 1; }
	@echo "total_supply:"
	@$(STELLAR) contract invoke --id $(CONTRACT_ID) --source-account $(SOURCE_ACCOUNT) \
		--network $(NETWORK) -- get_total_supply
	@echo "is_paused:"
	@$(STELLAR) contract invoke --id $(CONTRACT_ID) --source-account $(SOURCE_ACCOUNT) \
		--network $(NETWORK) -- is_paused
	@echo "asset_status:"
	@$(STELLAR) contract invoke --id $(CONTRACT_ID) --source-account $(SOURCE_ACCOUNT) \
		--network $(NETWORK) -- get_asset_status

## verify-interface: Print the deployed contract's interface from the network.
verify-interface: check-cli
	@test -n "$(CONTRACT_ID)" || { echo "ERROR: CONTRACT_ID is not set."; exit 1; }
	$(STELLAR) contract info interface --id $(CONTRACT_ID) --network $(NETWORK)
