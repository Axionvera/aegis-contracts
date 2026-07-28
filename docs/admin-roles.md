# Admin Role Permission Controls

This document describes the role-based access control (RBAC) system for the Aegis RWA Protocol smart contracts.

## Role Hierarchy

| Role | Description | Privileged Operations |
|---|---|---|
| `Admin` | Supreme authority. Can perform all operations and manage roles. | All operations + role management + admin transfer |
| `ComplianceOfficer` | Manages the compliance whitelist. | `whitelist_user`, `revoke_whitelist` |
| `AssetManager` | Manages asset minting and yield distribution. | `mint_asset`, `distribute_yield` |
| `EmergencyOfficer` | Combined compliance + asset privileges for operational flexibility. | `whitelist_user`, `revoke_whitelist`, `mint_asset`, `distribute_yield` |
| `None` | No role assigned. Cannot perform any privileged operation. | None (except `transfer` which requires self-auth) |

### Admin Bypass

The supreme admin (`DataKey::Admin`) bypasses all role checks. The `require_role` helper function checks if the caller holds the required role OR is the admin, and reverts if neither condition is met.

## Role Management API

### `set_role(env, admin, target, role)`

Assigns a role to `target`. Only the admin can call this.

- Requires admin authentication (`require_auth`)
- Requires the caller to be the current admin
- **Cannot** assign the `Admin` role — use `transfer_admin` for safe handoff
- Emits a `RoleAssignedEvent`

### `remove_role(env, admin, target)`

Revokes the role from `target`, setting it to `Role::None`. Only the admin can call this.

- Requires admin authentication
- Reverts if the target has no role assigned
- Emits a `RoleRevokedEvent`

### `get_role_of(env, address)`

Returns the role assigned to `address`, or `Role::None` if unassigned.

### `transfer_admin(env, admin, candidate)`

Initiates a 2-step admin transfer. Sets `candidate` as the pending new admin.

- Requires admin authentication
- Stores the candidate in `DataKey::AdminCandidate`
- Emits an `AdminTransferInitiatedEvent`

### `accept_admin(env, candidate)`

Completes a 2-step admin transfer. The candidate must call this to accept.

- Requires candidate authentication
- Reverts if no pending transfer exists or if the caller is not the candidate
- Transfers the `Admin` role from the previous admin to the candidate
- Emits an `AdminTransferredEvent`

### `renounce_admin(env, admin)`

The admin can renounce their own role. This is irreversible.

- Requires admin authentication
- Removes the admin from storage and sets their role to `Role::None`
- Emits an `AdminTransferredEvent` (self-renounced)

## Events

All role changes emit Soroban events for off-chain indexing and audit trails:

| Event | Topic | Payload |
|---|---|---|
| Contract initialized | `("contract_initialized",)` | `{ admin }` |
| Role assigned | `("role_assigned",)` | `{ admin, target, role }` |
| Role revoked | `("role_revoked",)` | `{ admin, target, role }` |
| Admin transfer initiated | `("admin_transfer_initiated",)` | `{ current_admin, candidate }` |
| Admin transferred | `("admin_transferred",)` | `{ previous_admin, new_admin }` |
| Admin renounced | `("admin_renounced",)` | `{ previous_admin, new_admin }` |
| Contract paused | `("contract_paused",)` | `{ admin }` |
| Contract unpaused | `("contract_unpaused",)` | `{ admin }` |

See [`events.md`](events.md) for the full event schema reference and
SDK/dashboard compatibility notes.

## Role Assignment and Revocation Policy

### Initial Setup

1. The `initialize` function sets the first admin and grants them the `Admin` role.
2. The admin should then assign roles to trusted addresses using `set_role`.

### Ongoing Operations

- **Assigning roles**: The admin calls `set_role` with the target address and desired role. Each address can hold exactly one role.
- **Revoking roles**: The admin calls `remove_role` to revoke a role. The target reverts to `Role::None`.
- **Role upgrades**: To change a role, revoke the current role first, then assign the new one.

### Admin Transfer

The 2-step transfer pattern prevents accidental or malicious admin loss:

1. **Step 1**: Current admin calls `transfer_admin(admin, candidate)`.
2. **Step 2**: The candidate calls `accept_admin(candidate)`.

If the candidate does not accept, the transfer can be superseded by a new `transfer_admin` call with a different candidate.

### Emergency Procedures

- The admin can immediately revoke any role using `remove_role`.
- The admin can renounce their own role using `renounce_admin` (irreversible).
- In case of a compromised admin key, the admin should transfer to a new key immediately.

## Privileged Operation Requirements

| Operation | Required Role | Admin Bypass |
|---|---|---|
| `mint_asset` | `AssetManager` | Yes |
| `distribute_yield` | `AssetManager` | Yes |
| `whitelist_user` | `ComplianceOfficer` | Yes |
| `revoke_whitelist` | `ComplianceOfficer` | Yes |
| `set_role` | `Admin` | N/A (admin-only) |
| `remove_role` | `Admin` | N/A (admin-only) |
| `transfer` | Self-auth | N/A |
| `transfer_admin` | `Admin` | N/A (admin-only) |
| `accept_admin` | Candidate auth | N/A |

## Storage Layout

| Key | Storage Type | Description |
|---|---|---|
| `DataKey::Admin` | Instance | The supreme admin address |
| `DataKey::AdminCandidate` | Instance | Pending admin during 2-step transfer |
| `DataKey::Role(Address)` | Persistent | The role assigned to an address |
| `DataKey::Whitelist(Address)` | Persistent | Whitelist flag (legacy, kept for compatibility) |
| `DataKey::Balance(Address)` | Persistent | Token balance |
| `DataKey::TotalSupply` | Instance | Global total supply |
