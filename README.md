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

### SDK integration fixtures
Deterministic example outputs for downstream SDK, dashboard, and indexer
repos live in [`fixtures/sdk/`](fixtures/sdk). They are generated from real
contract invocations and re-verified on every test run, so contract drift
fails CI here instead of reaching consumers:
   ```bash
   make test-fixtures          # verify committed fixtures still match
   make update-fixtures        # regenerate after an intentional change
   ```
See [SDK Integration Fixtures](docs/sdk-fixtures.md) for the format, the
value-encoding rules, and the no-real-user-data guarantee.
## Security & Compliance

> **Important**: Please read our [Legal Boundary Disclaimer](docs/legal-boundary-disclaimer.md) to understand the off-chain assumptions and limitations of the RWA tokenization model.

- [RWA Protocol Threat Model](docs/threat-model.md) — protected assets, trust boundaries, threat catalog (compliance bypass, admin/role misuse, minting and transfer risks, pause misuse, event reliability), and explicit off-chain/legal out-of-scope items
- [Emergency Pause Policy](docs/emergency-pause.md) — global pause mechanism, authorization, and trust model
- [Admin Roles & Permissions](docs/admin-roles.md) — role-based access control (RBAC) design
- [Admin Misuse Risks](docs/admin-misuse-risks.md) — threat model and mitigations
- [Supply Cap Amendment Governance](docs/supply-cap-governance.md) — 2-step cap amendment workflow and enforcement

## Errors

- [Contract Error Code Standard](docs/error-codes.md) — standardized error codes for compliance, admin, minting, transfer, and storage failures, plus SDK/dashboard mapping guidance
- [SDK Integration Fixtures](docs/sdk-fixtures.md) — deterministic example outputs for compliance, minting, transfer, event, error, and capability scenarios, for cross-repo testing

- [Investor Holding Restriction Checks](docs/investor-holding-restrictions.md) — per-investor holding cap workflow and enforcement

## Investor Tooling

- [Investor Eligibility Read Helpers](docs/investor-eligibility.md) — compliance, holding-cap, and transfer eligibility read helpers for SDKs and dashboards

- [Dashboard Integration Readiness Review](docs/dashboard-readiness-review.md) — API gaps, event limitations, and SEP-41 token compatibility risks for front-end integrations
- [Dashboard Release Readiness Review (MVP)](docs/dashboard-release-readiness.md) — UI/UX gaps, test coverage requirements, and security flow risks for the dashboard application

- [Requirement Traceability Mapping](docs/traceability-mapping.md) — mandatory traceability table format for contract changes and PRs
- [Compliance Registry Reads and Indexing Strategy](docs/compliance-registry-reads.md) — supported point reads, event-indexed pagination, consistency guarantees, and dashboard/SDK boundaries
- [Dashboard Integration Readiness Review](docs/dashboard-readiness-review.md) — API gaps, event limitations, and SEP-41 token compatibility risks for front-end integrations
- [Dashboard Release Readiness Review (MVP)](docs/dashboard-release-readiness.md) — UI/UX gaps, test coverage requirements, and security flow risks for the dashboard application
- [Dashboard Local Troubleshooting Guide](docs/dashboard-troubleshooting.md) — practical fixes for Freighter setup, RPC errors, and Next.js configuration

## Contributor Guides

- [Local Deployment Guide](docs/local-deployment.md) — deployment assumptions, environment variables, Makefile reference, Soroban CLI usage, and common errors
- [Reviewer Checklist](docs/reviewer-checklist.md) — standardized quality and security checklist for PR reviewers
- [Contributor Experience Review](docs/contributor-experience-review.md) — known onboarding friction and follow-up items

## CI & Contributing

- [Local Verification Command](docs/local-verification.md) — run `make verify` (fmt-check + clippy + test + build) before pushing to avoid failing CI
- [Failing CI Response Guide](docs/failing-ci-guide.md) — how to reproduce and fix Rust, Soroban, Makefile, dependency, and workflow failures (failing checks can block approval)
- [Payment-Period Conduct Note](docs/payment-period-conduct.md) — contributor expectations during paid periods: no spam, self-review, GrantFox evaluation, CI/testing
- [Meaningful Implementation Checklist](docs/meaningful-implementation-checklist.md) — what counts as real contract work: behaviour, security, tests, events, acceptance criteria + reviewer checks


## Contributing
Please see CONTRIBUTING.md for guidelines on how to submit pull requests, branch naming conventions, and testing requirements.