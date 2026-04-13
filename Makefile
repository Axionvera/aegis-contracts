default: build

build:
	cargo build --target wasm32-unknown-unknown --release
	@echo "Build successful. WASM located in target/wasm32-unknown-unknown/release/"

test:
	cargo test

fmt:
	cargo fmt --all

clean:
	cargo clean

optimize: build
	soroban contract optimize --wasm target/wasm32-unknown-unknown/release/aegis_contracts.wasm