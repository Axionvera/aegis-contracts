# Advanced Development & Testing Guide

This guide covers everything required to build, test, debug, and extend advanced contract logic for the Aegis RWA Protocol.

## 1. Prerequisites

Before starting local development, ensure you have the following installed:
- **Rust** (>= 1.71).
- **Soroban CLI** - Used for optimizing and deploying contracts.
- **Node.js & npm** - Required for running the off-chain monitoring service tests.
- **WASM Target**: `wasm32v1-none` (automatically installed via `make build`).

> **Note**: soroban-sdk requires `wasm32v1-none`. The older `wasm32-unknown-unknown` target is rejected by the Soroban environment on Rust 1.82+.

## 2. Build Commands

We use a `Makefile` to simplify common operations.

- `make build`
  - Ensures the correct WASM target is added.
  - Builds the contract for the `wasm32v1-none` target in release mode.
  - Output is located at `target/wasm32v1-none/release/aegis_contracts.wasm`.
- `make optimize`
  - Builds and then runs `stellar contract optimize` to reduce the WASM binary size.
- `make clean`
  - Cleans the Rust project and removes monitoring service dependencies/data.

## 3. Test Structure

Tests are primarily located in `src/test.rs`. The test harness relies on the `soroban-sdk::testutils` feature to simulate a Soroban environment locally.

Key aspects of the testing environment:
- **`Env::default()`**: Creates a new, isolated test environment for each test.
- **`env.mock_all_auths()`**: Simplifies authorization checks for testing core logic without signing real transactions.
- **XDR Assertions**: State changes emit events that are strictly verified against expected XDR topics and data. Note that `Env::events().all()` is scoped to the most recent invocation. To test lifecycle events, they must be accumulated across calls.

## 4. Deterministic Fixtures

For off-chain compatibility (e.g., the Node.js monitoring service), we maintain deterministic XDR output:

- **Generating Fixtures**: Run `make dump-events` (or `cargo test dump_event_xdr -- --ignored --nocapture`) to dump the real, host-produced XDR for every Aegis event as base64.
- This ensures that the off-chain monitoring decoder can be verified against genuine contract output without spinning up a live network.

## 5. Compliance Tests

The protocol enforces regulatory requirements (e.g., KYC whitelisting) on-chain. Compliance tests verify these invariant rules:

- **Whitelist Enforcement**: Asserting `should_panic(expected = "Receiver is not whitelisted")` for mints or transfers to unapproved addresses.
- **Balance Limits**: Validating `should_panic(expected = "Insufficient balance")`.

When adding new compliance checks, ensure you write a corresponding `#[test]` with `#[should_panic(expected = "...")]` to guarantee unauthorized operations fail safely.

## 6. Admin Tests

Administrative actions (like initializing the contract, distributing yields, and updating whitelists) have restricted access.
- Tests simulating admin functions must verify that only the initial designated admin address can execute them.
- Any new administrative functionality should include tests validating both successful admin operations and failed unauthorized attempts.

## 7. Common Local Setup Errors

- **Target Reject Error**: "target rejected by the Soroban environment"
  - *Fix*: Make sure you are using `wasm32v1-none`. Running `make build` handles this automatically.
- **Missing XDR Events**: "event not found" in testing.
  - *Fix*: Remember that `Env::events().all()` only captures events from the **last** contract call in the test. You need to explicitly collect them if testing a sequence of calls.
- **Build Fails (Missing `stellar` CLI)**:
  - *Fix*: Install it via `cargo install --locked stellar-cli`. It's required for `make optimize`.

## 8. Contribution Expectations

We welcome contributions to the Aegis Protocol! Please follow these guidelines:
- **Test Coverage**: All new contract logic MUST be accompanied by unit tests in `src/test.rs` and `tests/`.
- **Code Formatting**: Run `make fmt` (which uses `cargo fmt`) before committing.
- **Documentation**: Update this guide or `docs/contract-spec.md` if changing the XDR event structures or contract interface.
- Refer to `CONTRIBUTING.md` for our branch naming conventions and PR process.
