#  Aegis RWA Contracts

Core smart contract infrastructure for the Aegis RWA Protocol. Built on the Stellar network using the Soroban SDK.

## Overview
Aegis enables the fractional tokenization of Real-World Assets (RWAs). The contracts strictly enforce regulatory compliance at the ledger level, ensuring tokens can only be minted to and transferred between KYC-whitelisted addresses.

## Prerequisites
* [Rust](https://rustup.rs/) (>= 1.71)
* [Soroban CLI](https://soroban.stellar.org/docs/getting-started/setup)

## Setup & Build
1. Install the Soroban WASM target (`make build` does this for you):
   ```bash
   rustup target add wasm32v1-none
   ```
   > soroban-sdk requires `wasm32v1-none`. The older `wasm32-unknown-unknown`
   > target is rejected by the Soroban environment on Rust 1.82+.
2. Build the contract:
   ```bash
   make build
   ```

## Testing
Run the comprehensive test suite locally:
   ```bash
   make test        # contract tests
   make test-all    # contract + monitoring service tests
   ```

## Real-Time Monitoring
Every state change emits a namespaced contract event (see
`docs/contract-spec.md`). The `.github/monitoring/` service consumes them over
Soroban RPC and provides live streaming, filtering and routing, pattern-based
alerting, event persistence and replay, an analytics dashboard, and
event-based triggers.

```bash
make monitor-install
make monitor-demo     # self-contained demo → http://127.0.0.1:4500
make monitor          # stream from a configured network
```

See [`.github/monitoring/README.md`](.github/monitoring/README.md) for
configuration and API details.

## Security & Auditing
Security reviewers start here: [`docs/audit-evidence-index.md`](docs/audit-evidence-index.md)
is the single index over all audit evidence — compliance enforcement, admin
roles, minting, transfers, asset metadata, storage layout, event schemas,
error catalogue, pause/migration status, and test coverage — with an honest
register of known gaps. Supporting material:

* [`docs/threat-model.md`](docs/threat-model.md) — assets, actors, threats, mitigations, residual risk
* [`docs/architecture.md`](docs/architecture.md) — module separation and storage tiers
* [`docs/contract-spec.md`](docs/contract-spec.md) — public API and emitted-event schema

Please report vulnerabilities privately before public disclosure.

## Contributing
Please see CONTRIBUTING.md for guidelines on how to submit pull requests, branch naming conventions, and testing requirements.