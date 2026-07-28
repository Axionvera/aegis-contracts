# Aegis RWA System Overview

This document provides a comprehensive analysis of the Aegis RWA smart contract codebase, covering its architecture, technical stack, data flows, and implementation details.

## 1. Project Architecture

The Aegis RWA protocol is built as a modular Stellar Soroban smart contract. The logic is partitioned into domain-specific modules to ensure separation of concerns and maintainability.

### Directory Structure
- [src/](file:///c:/Users/Muhammad/.trae/Grantfox/aegis-contracts/src): Core Rust source code.
    - [lib.rs](file:///c:/Users/Muhammad/.trae/Grantfox/aegis-contracts/src/lib.rs): Entry point, contract interface, and core types (Roles, DataKeys).
    - [admin.rs](file:///c:/Users/Muhammad/.trae/Grantfox/aegis-contracts/src/admin.rs): RBAC, Admin 2-step transfer, and Emergency Pause mechanism.
    - [asset.rs](file:///c:/Users/Muhammad/.trae/Grantfox/aegis-contracts/src/asset.rs): Token ledger (mint/transfer), asset status lifecycle, and metadata management.
    - [compliance.rs](file:///c:/Users/Muhammad/.trae/Grantfox/aegis-contracts/src/compliance.rs): Whitelist (ACL) registry management.
    - [supply_cap.rs](file:///c:/Users/Muhammad/.trae/Grantfox/aegis-contracts/src/supply_cap.rs): Global supply cap governance (2-step amendment).
    - [holding.rs](file:///c:/Users/Muhammad/.trae/Grantfox/aegis-contracts/src/holding.rs): Per-investor holding restriction governance (2-step amendment).
    - [eligibility.rs](file:///c:/Users/Muhammad/.trae/Grantfox/aegis-contracts/src/eligibility.rs): Aggregated read-only helpers for investor eligibility.
    - [errors.rs](file:///c:/Users/Muhammad/.trae/Grantfox/aegis-contracts/src/errors.rs): Standardized error code definitions.
- [docs/](file:///c:/Users/Muhammad/.trae/Grantfox/aegis-contracts/docs): Extensive technical and operational documentation.
- [scripts/](file:///c:/Users/Muhammad/.trae/Grantfox/aegis-contracts/scripts): Deployment and local environment setup scripts.

## 2. Technical Stack

- **Language**: Rust (Edition 2021).
- **Framework**: [Soroban SDK](https://soroban.stellar.org/) (v26.0.0).
- **Target Network**: Stellar Network (Soroban WASM runtime).
- **Automation**: `Makefile` for building, testing, linting, and deployment.
- **CLI Tooling**: `Stellar CLI` (formerly `Soroban CLI`) for network interaction.
- **CI/CD**: GitHub Actions for automated linting (`clippy`), formatting (`fmt`), and testing.

## 3. Core Logic & Data Flow

### Access Control & Safety
Every state-changing operation follows a strict verification pipeline:
1. **Authorization**: `require_auth()` ensures the caller is who they claim to be.
2. **Pause Check**: `require_not_paused()` blocks operations if the contract is in emergency mode.
3. **Role Check**: `require_role()` or `require_any_role()` verifies the caller has sufficient privileges (Admin, ComplianceOfficer, AssetManager, or EmergencyOfficer).

### Critical Execution Paths

#### Asset Minting
1. `AssetManager` initiates `mint_asset`.
2. Validates `Active` status and `Amount > 0`.
3. Verifies recipient is `Whitelisted`.
4. Enforces **Global Supply Cap** (reverts if exceeded).
5. Enforces **Investor Holding Cap** (reverts if recipient's resulting balance exceeds cap).
6. Updates `Balance` (Persistent) and `TotalSupply` (Instance).
7. Publishes `AssetMintedEvent`.

#### Asset Transfer
1. Sender initiates `transfer`.
2. Verifies both Sender and Recipient are `Whitelisted`.
3. Validates `Active` status and `Amount > 0`.
4. Enforces **Investor Holding Cap** for the recipient.
5. Checks Sender's balance.
6. Atomically updates both balances.
7. Publishes `TransferEvent`.

## 4. Components & State Management (Database Schema)

The contract utilizes Soroban's dual-class storage system:

| Component | State Keys | Storage Class | Purpose |
|---|---|---|---|
| **Governance** | `Admin`, `AdminCandidate`, `Paused` | Instance | Core protocol ownership and emergency state. |
| **RBAC** | `Role(Address)` | Persistent | Assigned roles for privileged operations. |
| **Compliance** | `Whitelist(Address)` | Persistent | KYC/AML whitelisting status for addresses. |
| **Ledger** | `Balance(Address)`, `TotalSupply` | Persistent/Instance | Token distribution and aggregate supply metrics. |
| **Governance Caps** | `SupplyCap`, `HoldingCap` + Candidates | Instance | Configurable protocol-level limits. |
| **Metadata** | `AssetName`, `AssetSymbol`, `AssetStatus` | Instance | Display info and lifecycle status. |

## 5. Architectural Patterns & Quality Standards

- **2-Step Governance**: Used for Admin transfers and Cap amendments to prevent accidental "bricking" of the contract via typos or malicious proposals.
- **Aggregated Read Helpers**: `get_investor_eligibility` combines multiple state checks (whitelist, balance, cap, pause) into one call to reduce RPC overhead for front-ends.
- **Monolith Interface**: While logic is modularized in Rust files, all public functions are exposed through a single `AegisContract` struct, providing a unified API for integrators.
- **Standardized Errors**: Uses a custom `Error` enum with explicit codes (e.g., `1001` for `Unauthorized`) for easier off-chain debugging.

## 6. Technical Debt & Known Limitations

- **Legacy Flags**: `DataKey::Whitelist` is marked as legacy in `lib.rs` but is currently the active storage key in `compliance.rs`.
- **Placeholder Logic**: `distribute_yield` is currently a mock. A scalable implementation (e.g., claim-based or snapshot-based) is required for production.
- **Missing Optimizations**:
    - Batch whitelisting/revocation is not yet implemented.
    - Fee deduction mechanism on transfer is a planned TODO.
- **Scalability**: Direct iteration over balances is avoided, but yield distribution needs a robust design to handle large numbers of holders without hitting gas limits.

## 7. Conclusion

Aegis RWA is a security-first protocol that prioritizes regulatory compliance at the ledger level. Its modular architecture and 2-step governance patterns provide a solid foundation for tokenizing Real-World Assets on the Stellar network.
