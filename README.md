#  Aegis RWA Contracts

Core smart contract infrastructure for the Aegis RWA Protocol. Built on the Stellar network using the Soroban SDK.

## Overview
Aegis enables the fractional tokenization of Real-World Assets (RWAs). The contracts strictly enforce regulatory compliance at the ledger level, ensuring tokens can only be minted to and transferred between KYC-whitelisted addresses.

## Prerequisites
* [Rust](https://rustup.rs/) (>= 1.84 — required by the `wasm32v1-none` target)
* [Stellar CLI](https://developers.stellar.org/docs/tools/cli/stellar-cli) (>= 21.0.0, the `stellar` binary; formerly `soroban`)
* [Docker](https://docs.docker.com/get-docker/) — only needed to run a local network

## Setup & Build
1. Install the WASM target:
   ```bash
   rustup target add wasm32v1-none
   ```
   > **Note:** `soroban-sdk` 26.x cannot build for `wasm32-unknown-unknown` on Rust 1.82+, so `wasm32v1-none` is the supported target. See the [Local Deployment Guide](docs/local-deployment.md#1-deployment-assumptions) for details.
2. Create your local configuration:
   ```bash
   cp .env.example .env
   ```
3. Build the contract:
   ```bash
   make build
   ```

## Testing
Run the comprehensive test suite locally. Tests run on your native target — no
WASM target, Docker, or network required:
   ```bash
   make test
   ```

## Local Deployment

Spin up a local Stellar network, deploy, and initialize the contract:

```bash
./scripts/setup_local.sh      # start local network + create/fund a deployer key
./scripts/deploy_local.sh     # build, deploy, and initialize
```

📖 **[Local Deployment Guide](docs/local-deployment.md)** — the complete
reference: deployment assumptions, every environment variable, all Makefile
targets, Soroban/Stellar CLI usage, and a troubleshooting catalog of common
errors.

## Makefile Commands

Run `make help` for this list plus your currently resolved configuration.

| Command | Description |
|---|---|
| `make build` | Compile the contract to WASM (`wasm32v1-none`) |
| `make test` | Run the full unit test suite |
| `make fmt` / `make fmt-check` | Format sources / verify formatting |
| `make clippy` | Lint with warnings as errors |
| `make ci` | Full PR gate: `fmt-check` + `clippy` + `test` + `build` |
| `make optimize` | Build and shrink the WASM for deployment |
| `make clean` | Remove build artifacts |
| `make network-up` / `make network-down` | Start / stop the local Stellar network |
| `make network-add` | Register the network alias in the Stellar CLI config |
| `make fund` | Create and fund the deployer identity |
| `make deploy` | Build and deploy to the configured network |
| `make initialize` | Call `initialize(admin)` on a deployed contract |
| `make invoke-status` | Read back supply, pause state, and asset status |
| `make verify` | Print the deployed contract's interface |

Every target honours the variables in `.env` and accepts inline overrides, e.g.
`make deploy NETWORK=testnet`. All variables are documented in
[the deployment guide](docs/local-deployment.md#4-environment-variables).
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

- [Contract Capability Flags](docs/capabilities.md) — read-only capability descriptor for feature gating: enabled modules and supported/planned/unsupported protocol behaviours across compliance, minting, transfers, pause, metadata, events, and versioning
- [Investor Eligibility Read Helpers](docs/investor-eligibility.md) — compliance, holding-cap, and transfer eligibility read helpers for SDKs and dashboards
- [Compliance Registry Reads and Indexing Strategy](docs/compliance-registry-reads.md) — supported point reads, event-indexed pagination, consistency guarantees, and dashboard/SDK boundaries
- [Dashboard Integration Readiness Review](docs/dashboard-readiness-review.md) — API gaps, event limitations, and SEP-41 token compatibility risks for front-end integrations
- [Dashboard Release Readiness Review (MVP)](docs/dashboard-release-readiness.md) — UI/UX gaps, test coverage requirements, and security flow risks for the dashboard application
- [Dashboard Local Troubleshooting Guide](docs/dashboard-troubleshooting.md) — practical fixes for Freighter setup, RPC errors, and Next.js configuration

## Contributor Guides

- [Local Deployment Guide](docs/local-deployment.md) — deployment assumptions, environment variables, Makefile reference, Soroban CLI usage, and common errors
- [Contributor Experience Review](docs/contributor-experience-review.md) — known onboarding friction and follow-up items

## CI & Contributing

- [Failing CI Response Guide](docs/failing-ci-guide.md) — how to reproduce and fix Rust, Soroban, Makefile, dependency, and workflow failures (failing checks can block approval)
- [Payment-Period Conduct Note](docs/payment-period-conduct.md) — contributor expectations during paid periods: no spam, self-review, GrantFox evaluation, CI/testing
- [Meaningful Implementation Checklist](docs/meaningful-implementation-checklist.md) — what counts as real contract work: behaviour, security, tests, events, acceptance criteria + reviewer checks

## Contributing
Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on how to submit pull requests, branch naming conventions, and testing requirements.