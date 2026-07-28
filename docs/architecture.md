# Protocol Architecture

## Separation of Concerns
The Aegis smart contract logic is cleanly modularized to separate state constraints from business logic:
* **`admin.rs`:** Handles role-based access control (RBAC), admin transfer, and role management. Provides the `require_role` helper used by all privileged operations.
* **`compliance.rs`:** Handles all Access Control Lists (ACL). Whitelist registries are managed here. Privileged operations require the ComplianceOfficer role.
* **`asset.rs`:** Handles mathematical balances and total supply management. It strictly queries the compliance module before executing state changes. Privileged operations require the AssetManager role.

## Role-Based Access Control

The contract implements a four-tier role system:

| Role | Operations |
|---|---|
| Admin | All operations + role management + admin transfer |
| ComplianceOfficer | whitelist_user, revoke_whitelist |
| AssetManager | mint_asset, distribute_yield |
| EmergencyOfficer | Combined compliance + asset privileges |

The supreme admin bypasses all role checks. Roles are assigned via `set_role` and revoked via `remove_role`. Admin transfer uses a 2-step pattern (`transfer_admin` / `accept_admin`) to prevent accidental loss.

## Ledger State Storage
Soroban utilizes three storage types. Aegis manages state as follows:
* **Instance Storage:** `Admin` address, `AdminCandidate` (during transfer), and `TotalSupply`. These are bound to the lifecycle of the contract instance.
* **Persistent Storage:** `Role` assignments, `Whitelist` status, and User `Balance`. These must persist independently and be rent-exempted appropriately to ensure user balances and roles are never archived unexpectedly.
