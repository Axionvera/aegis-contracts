default: build

build:
	cargo build --target wasm32-unknown-unknown --release
	@echo "Build successful. WASM located in target/wasm32-unknown-unknown/release/"

test:
	cargo test

# Verify the committed SDK integration fixtures still match live contract
# behaviour. Fails on drift; see docs/sdk-fixtures.md.
test-fixtures:
	cargo test --test sdk_fixtures

# Regenerate the committed fixtures after an intentional contract change.
# Review the resulting diff before committing.
update-fixtures:
	UPDATE_FIXTURES=1 cargo test --test sdk_fixtures
	@echo "Fixtures regenerated in fixtures/sdk/. Review the diff before committing."

fmt:
	cargo fmt --all

clean:
	cargo clean

optimize: build
	soroban contract optimize --wasm target/wasm32-unknown-unknown/release/aegis_contracts.wasm

.PHONY: default build test test-fixtures update-fixtures fmt clean optimize
