# Dashboard Release Readiness Review (MVP)

## Overview
This document serves as the release readiness checklist for the Aegis RWA Dashboard MVP. It evaluates the dashboard's ability to safely interact with the Aegis smart contracts, identifying known UX gaps, missing test coverage, and configuration risks.

> **Note on MVP Limitations:** The dashboard relies on a smart contract that does not currently implement SEP-41 (no on-chain `name`, `symbol`, or `decimals`). The dashboard MVP must hardcode these values and is not a generalized token explorer.

## Release Readiness Evaluation

### 1. Wallet & Connection Flows
| Risk Area | Severity | Status / Gap | Description |
| :--- | :--- | :--- | :--- |
| **Network Configuration** | High | ⚠️ Risk | The dashboard must ensure it connects to the same network (e.g., Futurenet/Testnet) as the deployed contracts. Misconfiguration will result in cryptic RPC failures. |
| **Freighter / Albedo Support** | High | ⚠️ Risk | The dashboard must handle wallet disconnection or lack of permissions gracefully, rather than crashing when attempting to sign a `transfer`. |

### 2. Compliance & Investor UX
| Risk Area | Severity | Status / Gap | Description |
| :--- | :--- | :--- | :--- |
| **Error Handling (4000/4001)** | High | ❌ Gap | When a transfer reverts due to compliance (e.g. Receiver Not Whitelisted), the dashboard currently relies on raw `Error(Contract, 4001)` strings. The dashboard must implement a mapping layer to surface human-readable errors to the user. |
| **Simulation Race Conditions** | Medium | ⚠️ Risk | The dashboard relies on `check_transfer_eligibility()`. If an investor's whitelist status is revoked *after* the dashboard simulates the transaction but *before* submission, the UI will falsely indicate success but the transaction will fail. |
| **Holding Cap Indicators** | Low | ❌ Gap | `get_investor_eligibility` returns `remaining_capacity`, but the UI does not currently visualize this headroom effectively during a transfer input, potentially allowing users to input amounts that will fail. |

### 3. Minting & Asset Views
| Risk Area | Severity | Status / Gap | Description |
| :--- | :--- | :--- | :--- |
| **Hardcoded Metadata** | Medium | ⚠️ Risk | Because the contract lacks `name()` and `decimals()`, the frontend must hardcode decimal formatting (e.g., assuming 7 decimals). This is acceptable for MVP but poses a risk if the contract deployment changes scaling logic. |
| **Event Polling** | Medium | ❌ Gap | The dashboard lacks a reliable event-polling service for `asset_minted` and `transfer` events, meaning transaction history might rely on slower manual RPC horizon queries. |

### 4. Security-Sensitive Flows
| Risk Area | Severity | Status / Gap | Description |
| :--- | :--- | :--- | :--- |
| **Admin vs. Investor Views** | High | ⚠️ Risk | The dashboard must verify the user's role via `get_role_of()` and strictly hide admin controls (like `pause` or `whitelist_user`) from regular investors to prevent UX confusion. |
| **Emergency Pause UX** | High | ❌ Gap | If `is_paused()` returns true, the dashboard must disable all "Send", "Mint", and "Whitelist" buttons globally, showing a clear maintenance banner. Currently, users can attempt to submit transactions that will predictably fail with error `3004`. |

### 5. Missing Dashboard Tests
| Test Category | Severity | Gap Description |
| :--- | :--- | :--- |
| **E2E Component Tests** | High | Missing Playwright/Cypress coverage for the "Connect Wallet -> View Balance -> Attempt Transfer" flow. |
| **Error State Tests** | Medium | Missing unit tests in `src/features/` or `src/components/` ensuring that raw `Error(Contract, 4000)` responses are properly parsed and mapped to the UI error boundary. |

---

## Follow-up Recommendations
Before advancing beyond MVP, the dashboard repository should address the following:
1. **Implement Error Mapping Layer:** Build a robust Soroban RPC error parser in the dashboard's service layer that maps the codes defined in `error-codes.md`.
2. **Add E2E Tests:** Introduce Playwright tests mocking the Soroban RPC responses for eligible and ineligible users.
3. **Handle Pause State:** Implement a global context provider in the React/Vue app that polls `is_paused()` and globally disables interactive transaction buttons when true.
