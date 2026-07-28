# Protocol Architecture

## Separation of Concerns
The Aegis smart contract logic is cleanly modularized to separate state constraints from business logic:
* **`admin.rs`:** Handles role-based access control (RBAC), admin transfer, role management, and the emergency pause mechanism. Provides the `require_role` and `require_not_paused` helpers used by all privileged operations.
* **`compliance.rs`:** Handles all Access Control Lists (ACL). Whitelist registries are managed here. Privileged operations require the ComplianceOfficer role.
* **`asset.rs`:** Handles mathematical balances and total supply management. It strictly queries the compliance module before executing state changes. Privileged operations require the AssetManager role.
* **`eligibility.rs`:** Read-only composition of the compliance, holding-cap, and pause state into per-investor eligibility answers. Introduces no storage of its own.
* **`capabilities.rs`:** Read-only capability descriptor advertising which modules are enabled and which protocol behaviours are supported, for SDK/dashboard feature gating. Introduces no storage of its own and reads through each module's own helper, so it cannot drift from the behaviour it describes. See [Contract Capability Flags](capabilities.md).

## Role-Based Access Control

The contract implements a four-tier role system:

| Role | Operations |
|---|---|
| Admin | All operations + role management + admin transfer + unpause |
| ComplianceOfficer | whitelist_user, revoke_whitelist |
| AssetManager | mint_asset, distribute_yield |
| EmergencyOfficer | Combined compliance + asset privileges + pause |

The supreme admin bypasses all role checks. Roles are assigned via `set_role` and revoked via `remove_role`. Admin transfer uses a 2-step pattern (`transfer_admin` / `accept_admin`) to prevent accidental loss.

## Emergency Pause

A global pause mechanism blocks all state-changing operations when activated. See [Emergency Pause Policy](emergency-pause.md) for full details.

* **Who can pause:** Admin or EmergencyOfficer
* **Who can unpause:** Admin only
* **What is blocked:** minting, transfers, compliance changes, role management
* **What remains available:** read functions (`get_role_of`, `get_balance_of`, `get_total_supply`, `is_whitelisted`, `is_paused`, `get_investor_eligibility`, `check_transfer_eligibility`, `get_capabilities`, `supports_capability`, `get_capability_keys`)

## Ledger State Storage
Soroban utilizes three storage types. Aegis manages state as follows:
* **Instance Storage:** `Admin` address, `AdminCandidate` (during transfer), `TotalSupply`, and `Paused` flag. These are bound to the lifecycle of the contract instance.
* **Persistent Storage:** `Role` assignments, `Whitelist` status, and User `Balance`. These must persist independently and be rent-exempted appropriately to ensure user balances and roles are never archived unexpectedly.
