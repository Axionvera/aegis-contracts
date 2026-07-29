# Aegis RWA Protocol — Audit Readiness Review

**Document Version:** 1.0  
**Date:** 2026-07-29  
**Target Repository:** `onakijames-droid/aegis-contracts`  
**Scope:** Core Soroban RWA smart contracts (`src/`) and real-time event monitoring tier (`.github/monitoring/`)  

---

## Executive Summary

This Audit Readiness Review evaluates the Aegis Real-World Asset (RWA) protocol smart contracts and supporting off-chain infrastructure prior to submitting the codebase for formal third-party security audits. 

The Aegis RWA protocol is designed to enforce regulatory compliance at the ledger level on Stellar/Soroban, restricting token issuance and transfers to KYC-whitelisted addresses while providing real-time off-chain event monitoring. While the protocol demonstrates a clean separation of concerns and robust event emissions, **this audit readiness review identifies several critical security blockers, unresolved design decisions, and missing test suites that must be addressed before production deployment or external audit**.

### Key Review Metrics
* **Security Blockers Identified:** 4 High-Risk / Blocker Issues
* **Unresolved Design Decisions & Architectural Limitations:** 6 Core Decisions
* **Test Coverage Gap:** 11 Recommended Security Test Cases Missing
* **Off-Chain / SDK Risks:** 5 Integration & Dashboard Risk Areas

---

## 1. Scope & Methodology

The review covered the entire Aegis protocol repository, examining contract logic, storage schemas, compliance constraints, event formatting, and off-chain streaming compatibility:

| File / Component | Type | Responsibility | Status |
| :--- | :--- | :--- | :--- |
| `src/lib.rs` | Smart Contract | Instance initialization & storage keys (`DataKey`) | Reviewed — High Risk (Admin revocation/two-step transfer missing) |
| `src/compliance.rs` | Smart Contract | KYC whitelist ACL & compliance check helpers | Reviewed — Blocker (No KYC revocation or pause) |
| `src/asset.rs` | Smart Contract | Minting, peer-to-peer transfers, yield distribution | Reviewed — Blocker (No burn, pause, or real yield logic) |
| `src/events.rs` | Smart Contract | Canonical structured event schemas (`#[contractevent]`) | Reviewed — Well Structured |
| `src/test.rs` | Unit Tests | Lifecycle and event emission test suite | Reviewed — Major Coverage Gaps |
| `.github/monitoring/` | Off-Chain SDK/Service | Event streaming, normalization, alerts, dashboard | Reviewed — SDK/RPC & Reorg Risks Identified |

---

## 2. High-Risk Areas & Security Blockers

The following issues represent immediate security or regulatory compliance blockers that must be resolved prior to formal audit:

### [BLK-01] Missing KYC Revocation (`whitelist_remove` / Blacklist) & Frozen Asset Handling
* **Severity:** **CRITICAL / REGULATORY BLOCKER**
* **Location:** `src/compliance.rs`
* **Description:** Currently, `whitelist_user` only sets `DataKey::Whitelist(user)` to `true`. There is no mechanism to remove an address from the whitelist (`whitelist_remove`), suspend a compromised account, or freeze assets held by a sanctioned entity.
* **Impact:** In RWA protocols, regulatory authorities (e.g., OFAC, SEC, FinCEN) require the ability to freeze assets or revoke KYC status when an investor fails ongoing compliance checks or is sanctioned. Without account freezing or KYC removal, non-compliant actors can continue holding or transferring tokens if they were whitelisted once.
* **Remediation:** Implement `remove_whitelist_user(env, admin, user)` and an optional `freeze_user(env, admin, user)` function. Update `transfer` to assert that neither sender nor receiver is frozen.

### [BLK-02] Absence of Emergency Pause / Circuit Breaker Mechanism
* **Severity:** **HIGH**
* **Location:** `src/lib.rs`, `src/asset.rs`, `src/compliance.rs`
* **Description:** The protocol lacks a global or modular pause mechanism (`is_paused` flag in Instance storage).
* **Impact:** In the event of a smart contract vulnerability, bridge compromise, or regulatory injunction, the administrator has no way to halt `mint_asset`, `transfer`, or `whitelist_user` operations.
* **Remediation:** Introduce a `DataKey::Paused` boolean flag in instance storage. Add an `admin_pause(env, admin, paused)` endpoint and an `assert!(!is_paused(&env))` modifier/check across all state-mutating functions.

### [BLK-03] Missing Token Burn & Clawback Capabilities for Regulatory Recovery
* **Severity:** **HIGH**
* **Location:** `src/asset.rs`
* **Description:** While `mint_asset` increases user balances and `TotalSupply`, there is no `burn` or administrative `clawback` function.
* **Impact:** Legal asset issuers must be able to burn tokens (e.g., upon fiat redemption/off-ramping) and execute regulatory clawbacks when courts order the recovery of stolen or disputed real-world assets.
* **Remediation:** Implement `burn(env, from, amount)` for user redemptions and `clawback(env, admin, from, to, amount)` for court-ordered administrative transfers.

### [BLK-04] Single-Step Admin Ownership & Lack of Role-Based Access Control (RBAC)
* **Severity:** **HIGH**
* **Location:** `src/lib.rs`, `src/compliance.rs`, `src/asset.rs`
* **Description:** The contract relies on a single `DataKey::Admin` set during `initialize`. There is no two-step ownership transfer (`propose_admin` / `claim_admin`), nor is there separation between compliance officers and minting operators.
* **Impact:** If the single admin private key is compromised or lost, the entire contract is permanently lost or controlled by an attacker. Furthermore, compliance teams whitelisting users should not require the same super-admin key responsible for minting assets.
* **Remediation:** Implement two-step admin transfer and separate roles into `Admin`, `ComplianceManager`, and `AssetIssuer`.

---

## 3. Unresolved Design Decisions & Explicit Limitations

The repository contains several explicit `// TODO:` markers, assumptions, and incomplete features that must be formally decided before audit:

### [DEC-01] Yield Distribution Scalability (`distribute_yield`)
* **Status:** **UNRESOLVED / MOCK IMPLEMENTATION**
* **Location:** `src/asset.rs:104`
* **Description:** The function `distribute_yield` currently acts as an event-only mock (`// TODO: Implement scalable yield snapshot mechanism`).
* **Design Problem:** Soroban contracts cannot iterate over unbounded persistent storage maps (all token holders) to distribute dividends without exceeding ledger gas limits.
* **Decision Needed:** Decide between:
  1. **Pull-Based Dividend Accounting (Recommended):** Store cumulative dividend per share (`accumulated_reward_per_share`) and let users claim dividends individually.
  2. **Snapshot / Off-Chain Distribution:** Emit snapshot events and distribute yield via batch off-chain claim trees or merkle proofs.

### [DEC-02] Transfer Fee Deduction Mechanics
* **Status:** **UNRESOLVED**
* **Location:** `src/asset.rs:69` (`// TODO: Implement fee deduction on transfer`)
* **Description:** A marker indicates planned fee deductions during transfers.
* **Decision Needed:** Define whether transfer fees are burned, sent to a treasury address, or retained by an asset manager, and ensure fee calculations do not introduce rounding vulnerabilities for small transfer amounts.

### [DEC-03] Batch Whitelisting & Gas Efficiency
* **Status:** **UNRESOLVED**
* **Location:** `src/compliance.rs:16` (`// TODO: Implement batch whitelisting to save gas`)
* **Description:** Whitelisting requires one transaction per user.
* **Decision Needed:** Evaluate whether to implement `batch_whitelist_users(env, admin, users: Vec<Address>)` or rely on off-chain signature-based compliance permits (SEP-41 / EIP-2612 style authorization).

### [DEC-04] Storage TTL & Rent-Exemption Archival Strategy
* **Status:** **EXPLICIT LIMITATION**
* **Location:** `src/asset.rs`, `src/compliance.rs`
* **Description:** While `docs/architecture.md` notes that Persistent storage must be rent-exempted appropriately, the code does not explicitly invoke `env.storage().persistent().extend_ttl(...)` when reading or writing user balances and whitelist flags.
* **Decision Needed:** Establish a uniform TTL extension policy (e.g., bumping storage entries by 30–90 days on every interaction) to prevent user balances or whitelist states from being archived on public Stellar mainnet.

### [DEC-05] Legal & Regulatory Assumptions
* **Status:** **EXPLICIT LEGAL ASSUMPTIONS**
* **Description:** 
  * The protocol assumes all KYC/AML identity verification occurs off-chain by an authorized issuer prior to calling `whitelist_user`.
  * The protocol assumes 1:1 parity or legally binding backing between on-chain tokens and off-chain real-world assets; smart contracts cannot verify physical asset custody.

---

## 4. Missing Test Coverage

The current unit test suite in `src/test.rs` provides 9 tests covering basic happy paths, whitelist rejection, zero-amount assertions, and event decoding. To be audit-ready, the test suite must be expanded to cover the following missing security and boundary scenarios:

| Test Case ID | Test Name | Objective & Security Verification |
| :--- | :--- | :--- |
| `TST-01` | `test_reinitialize_fails` | Verify that calling `initialize()` a second time aborts with `"Contract already initialized"`. |
| `TST-02` | `test_unauthorized_whitelist_fails` | Verify that a non-admin caller cannot invoke `whitelist_user`. |
| `TST-03` | `test_unauthorized_mint_fails` | Verify that a non-admin caller cannot invoke `mint_asset`. |
| `TST-04` | `test_mint_to_non_whitelisted_fails` | Ensure `mint_asset` aborts when the target address is not whitelisted. |
| `TST-05` | `test_transfer_from_non_whitelisted_fails` | Ensure `transfer` aborts if the sender's KYC status is missing/removed. |
| `TST-06` | `test_transfer_to_non_whitelisted_fails` | Ensure `transfer` aborts if the recipient's KYC status is missing/removed. |
| `TST-07` | `test_transfer_insufficient_balance_fails` | Ensure transfers exceeding user balance revert safely without arithmetic overflow. |
| `TST-08` | `test_arithmetic_overflow_protection` | Ensure minting or transferring near `i128::MAX` is safely handled by Rust/Soroban checked math. |
| `TST-09` | `test_self_transfer` | Verify behavior when `from == to` during a transfer (ensure balance remains invariant). |
| `TST-10` | `test_whitelist_removal_workflow` | *(After implementing `BLK-01`)* Verify that removing an address from the whitelist immediately blocks pending transfers. |
| `TST-11` | `test_pause_unpause_workflow` | *(After implementing `BLK-02`)* Verify that pausing the contract disables mints and transfers, and unpausing restores them. |

> **Note on Auth Mocking:** Current tests rely heavily on `env.mock_all_auths()`. Security tests should also include strict authentication assertions (`env.mock_auths(...)`) to verify exact cryptographic authorization boundaries.

---

## 5. SDK Compatibility & Off-Chain Dashboard Readiness

The off-chain monitoring service (`.github/monitoring/`) consumes Soroban RPC events and exposes an analytics dashboard. The following cross-repo and SDK integration risks must be mitigated:

### [SDK-01] Public Soroban RPC WebSocket Subscription Absence
* **Risk Level:** **HIGH / ARCHITECTURAL LIMITATION**
* **Description:** As noted in `docs/architecture.md`, public Soroban RPC endpoints do not currently provide native WebSocket subscription APIs (`subscribeEvents`).
* **Operational Impact:** The monitoring client (`websocket-client.js`) transparently falls back to cursor-driven polling via `getEvents`. When deploying against high-throughput public RPC nodes, aggressive polling intervals can lead to rate-limiting (HTTP 429), delayed event delivery, or missed alerts.
* **Remediation:** Provide built-in exponential backoff with jitter on HTTP 429 responses and document recommended RPC provider configurations (e.g., dedicated QuickNode or ValidationCloud RPC instances).

### [SDK-02] Ledger Reorganization & Event Deduplication
* **Risk Level:** **MEDIUM**
* **Description:** The off-chain event store (`event-store.js`) indexes events by cursor and ledger sequence. If a Stellar/Soroban edge node experiences temporary fork resolution or ledger rollback, event streams may replay or re-emit events.
* **Operational Impact:** The dashboard analytics and alert triggers could double-count mints, transfers, or yield distributions unless strict idempotency keys (`ledger_sequence + tx_hash + event_index`) are enforced in the database tier.
* **Remediation:** Implement unique constraint deduplication in the monitoring persistence layer.

### [SDK-03] XDR Schema Drift Between On-Chain Events & JavaScript ScVal Decoder
* **Risk Level:** **MEDIUM**
* **Description:** The JavaScript XDR decoder (`scval.js`) parses `i128` values, tuples, and strkey addresses emitted by `#[contractevent]`.
* **Operational Impact:** If the contract event topic structure or data format evolves (e.g., adding fees or timestamps to `Transfer` or `Mint`), the JavaScript normalization layer will fail or misclassify payloads without compile-time type safety.
* **Remediation:** Implement automated CI contract-to-SDK compatibility tests (similar to `onchain-compat.test.js`) that validate generated XDR schemas against the JavaScript decoder on every contract PR.

### [SDK-04] SEP-41 / Token Standard Interoperability
* **Risk Level:** **LOW / DESIGN NOTE**
* **Description:** Aegis uses custom `("aegis", "transfer")` event topics rather than standard SEP-41 token topics.
* **Operational Impact:** Standard Stellar wallets and block explorers that solely listen for SEP-41 `transfer` topics may not automatically display Aegis RWA balance movements without custom indexing.
* **Remediation:** Maintain clear documentation in `docs/contract-spec.md` for third-party explorers on how to filter `aegis` namespace events.

---

## 6. Recommended Action Plan for Audit Readiness

1. **Phase 1: Security Hardening (Prior to Audit Submission)**
   * Implement KYC revocation (`remove_whitelist_user`) and asset freeze capabilities (`BLK-01`).
   * Add emergency contract pause/circuit breaker storage flags (`BLK-02`).
   * Add administrative token burn and clawback functions (`BLK-03`).
   * Implement two-step admin transfer and separate compliance/minting roles (`BLK-04`).

2. **Phase 2: Architectural Decision Resolution**
   * Replace the `distribute_yield` mock with a pull-based dividend accounting schema (`DEC-01`).
   * Implement explicit Soroban storage TTL bump calls (`extend_ttl`) for persistent maps (`DEC-04`).

3. **Phase 3: Test Suite & SDK Verification**
   * Add the 11 missing security unit test cases (`TST-01` to `TST-11`) to `src/test.rs`.
   * Ensure strict auth testing without `mock_all_auths()` for privileged functions.
   * Add RPC rate-limiting handling and idempotency checks to the monitoring service (`SDK-01`, `SDK-02`).

---

*This audit readiness review provides a comprehensive baseline of technical risks, architectural dependencies, and compliance limitations. Addressing these findings will ensure a smooth, cost-effective formal security audit and robust production deployment.*
