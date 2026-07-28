#  Aegis RWA Contracts

Core smart contract infrastructure for the Aegis RWA Protocol. Built on the Stellar network using the Soroban SDK.

## Overview
Aegis enables the fractional tokenization of Real-World Assets (RWAs). The contracts strictly enforce regulatory compliance at the ledger level, ensuring tokens can only be minted to and transferred between KYC-whitelisted addresses.

## Prerequisites
* [Rust](https://rustup.rs/) (>= 1.71)
* [Soroban CLI](https://soroban.stellar.org/docs/getting-started/setup)

## Setup & Build
1. Install the `wasm32-unknown-unknown` target:
   ```bash
   rustup target add wasm32-unknown-unknown
   ```
2. Build the contract:
   ```bash
   make build
   ```
   
## Testing
Run the comprehensive test suite locally:
   ```bash
   make test
   ```
## Security

- [Emergency Pause Policy](docs/emergency-pause.md) — global pause mechanism, authorization, and trust model
- [Admin Roles & Permissions](docs/admin-roles.md) — role-based access control (RBAC) design
- [Admin Misuse Risks](docs/admin-misuse-risks.md) — threat model and mitigations
- [Storage Audit Map](docs/storage-audit-map.md) — complete storage key reference, invariants, and test coverage

## Contributing
Please see CONTRIBUTING.md for guidelines on how to submit pull requests, branch naming conventions, and testing requirements.