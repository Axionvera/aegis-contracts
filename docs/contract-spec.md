# Contract API Specification

## Initialization

* `initialize(env, admin)`: Sets the initial admin and grants them the Admin role. Reverts if already initialized.

## Role Management (admin.rs)

* `set_role(env, admin, target, role)`: Assigns a role to `target`. Requires Admin role. Cannot assign the Admin role — use `transfer_admin` instead. Emits `RoleAssignedEvent`.
* `remove_role(env, admin, target)`: Revokes the role from `target`. Requires Admin role. Reverts if target has no role. Emits `RoleRevokedEvent`.
* `get_role_of(env, address)`: Returns the role assigned to `address`, or `Role::None`.
* `transfer_admin(env, admin, candidate)`: Initiates a 2-step admin transfer. Requires Admin role. Emits `AdminTransferInitiatedEvent`.
* `accept_admin(env, candidate)`: Completes a 2-step admin transfer. Only the pending candidate can call this. Emits `AdminTransferredEvent`.
* `renounce_admin(env, admin)`: The admin renounces their own role. Irreversible. Emits `AdminTransferredEvent`.

## Compliance (compliance.rs)

* `whitelist_user(env, admin, user)`: Adds `user` to the persistent compliance map. Requires ComplianceOfficer role (or Admin).
* `revoke_whitelist(env, admin, user)`: Removes `user` from the compliance whitelist. Requires ComplianceOfficer role (or Admin).

## Asset Operations (asset.rs)

* `mint_asset(env, admin, to, amount)`: Mints `amount` to `to`. Requires AssetManager role (or Admin). Reverts if `to` is not whitelisted.
* `transfer(env, from, to, amount)`: Moves `amount` between addresses. Requires `from` auth. Reverts if either `from` or `to` is not whitelisted, or if `from` has an insufficient balance.
* `distribute_yield(env, admin, amount)`: Triggers a dividend yield event for off-chain indexing. Requires AssetManager role (or Admin).
