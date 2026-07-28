# Dashboard Integration Readiness Review

## Overview
This document evaluates the readiness of the Aegis RWA Contracts for integration with a front-end dashboard or decentralized application (dApp). It maps expected dashboard user flows to the current smart contract capabilities, identifying critical API gaps, missing events, and configuration risks.

## Dashboard Use Cases vs. Contract Capabilities

| Use Case | Required Functionality | Current Contract Support | Status |
| :--- | :--- | :--- | :--- |
| **Portfolio & Balance View** | Fetch user balance, holding capacity, and pause state. | `get_investor_eligibility()` provides a composed, read-only response with balance, whitelist status, remaining holding capacity, and global pause state. | ✅ Supported |
| **Pre-flight Transfer Validation** | Check if a user's transfer will fail before submission. | `check_transfer_eligibility()` provides point-in-time checks for whitelist status, pause state, and holding caps. | ✅ Supported |
| **Asset Statistics** | Display the total minted supply of the asset. | `get_total_supply()` | ✅ Supported |
| **Feature Gating / Capability Discovery** | Determine which contract features exist before rendering UI, so unsupported controls are hidden rather than failing at submission. | `get_capabilities()`, `supports_capability()`, and `get_capability_keys()` expose a read-only descriptor covering compliance, minting, transfer, pause, metadata, event, and version support, with `Supported`/`Planned`/`Unsupported` states. See [`capabilities.md`](capabilities.md). | ✅ Supported |
| **Asset Token Metadata** | Display the token's name, symbol, and decimal precision. | *Missing.* No endpoints exist for `name()`, `symbol()`, or `decimals()`. | ❌ **Blocker** |
| **Third-Party Integrations / Approvals** | Allow other smart contracts or protocols to spend tokens on the user's behalf. | *Missing.* No `approve()`, `transfer_from()`, or `allowance()` endpoints exist. The contract is not fully SEP-41 compliant. | ❌ **Blocker** |

## Critical Gaps Identified

### 1. Missing APIs (No SEP-41 Token Compatibility)
The Aegis Contracts do **not** implement the standard Soroban Token Interface (SEP-41). 
- The absence of `name`, `symbol`, and `decimals` means a dashboard cannot dynamically render the asset; developers must hardcode these values.
- The absence of `approve` and `transfer_from` means standard wallets and DEX aggregators cannot seamlessly interact with this token.

### 2. Event and Error Observability Gaps
- **Missing Transfer Restriction Events:** When a transfer fails due to compliance rules (e.g., recipient not whitelisted), Soroban rolls back all state, including events. There is no durable on-chain event for a "rejected transfer". Dashboards must parse the raw `Error(Contract, 4000/4001)` from the transaction result.
- **No Asset Metadata Event:** There is currently no `asset_registered` event emitted to indexers, as the contract has no concept of internal asset metadata.

### 3. Configuration & Integration Risks
- **Simulation Race Conditions:** The `check_transfer_eligibility` helper is a point-in-time simulation. A dashboard might simulate a transfer successfully, but if a compliance officer revokes the user's whitelist status immediately after, the execution will still fail. The dashboard must be equipped to gracefully handle raw Soroban error codes `3004` (Paused), `4000` (Sender Not Whitelisted), and `4001` (Receiver Not Whitelisted) during actual submission.

## Follow-up Recommendations

To unblock dashboard integrations and minimize cross-repo delivery risk, the following issues should be prioritized:

1. **Implement SEP-41 Compatibility:** Refactor the contract to fully implement the Soroban `token::Interface` (including `approve`, `transfer_from`, `allowance`, `decimals`, `name`, and `symbol`). Until then, these gaps are machine-discoverable: `get_capabilities()` reports `sep41_token_interface`, `transfers.allowances`, `transfers.transfer_from`, and `metadata.decimals` as `Planned`, so a dashboard can hide or disable the corresponding controls instead of hardcoding the gap. Flip each to `Supported` in `capabilities.rs` as it ships.
2. **Standardize Error Code Parsing:** Ensure the dashboard's RPC client includes a robust error-mapping layer to translate `Error(Contract, 4000)` into a user-friendly "Recipient has not completed KYC" message, as detailed in `error-codes.md`.
