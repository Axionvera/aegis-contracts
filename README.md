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
## Security & Compliance

> **Important**: Please read our [Legal Boundary Disclaimer](docs/legal-boundary-disclaimer.md) to understand the off-chain assumptions and limitations of the RWA tokenization model.

- [RWA Protocol Threat Model](docs/threat-model.md) — protected assets, trust boundaries, threat catalog (compliance bypass, admin/role misuse, minting and transfer risks, pause misuse, event reliability), and explicit off-chain/legal out-of-scope items
- [Emergency Pause Policy](docs/emergency-pause.md) — global pause mechanism, authorization, and trust model
- [Admin Roles & Permissions](docs/admin-roles.md) — role-based access control (RBAC) design
- [Admin Misuse Risks](docs/admin-misuse-risks.md) — threat model and mitigations
- [Supply Cap Amendment Governance](docs/supply-cap-governance.md) — 2-step cap amendment workflow and enforcement

## Errors

- [Contract Error Code Standard](docs/error-codes.md) — standardized error codes for compliance, admin, minting, transfer, and storage failures, plus SDK/dashboard mapping guidance

- [Investor Holding Restriction Checks](docs/investor-holding-restrictions.md) — per-investor holding cap workflow and enforcement

## Investor Tooling

- [Investor Eligibility Read Helpers](docs/investor-eligibility.md) — compliance, holding-cap, and transfer eligibility read helpers for SDKs and dashboards
- [Dashboard Integration Readiness Review](docs/dashboard-readiness-review.md) — API gaps, event limitations, and SEP-41 token compatibility risks for front-end integrations
- [Dashboard Release Readiness Review (MVP)](docs/dashboard-release-readiness.md) — UI/UX gaps, test coverage requirements, and security flow risks for the dashboard application
- [Dashboard Local Troubleshooting Guide](docs/dashboard-troubleshooting.md) — practical fixes for Freighter setup, RPC errors, and Next.js configuration
## Contributing
Please see CONTRIBUTING.md for guidelines on how to submit pull requests, branch naming conventions, and testing requirements.