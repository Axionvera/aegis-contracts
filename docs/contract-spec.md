# Contract API Specification

> Every revert below is a standardized `Error` code, not a message string.
> See [`docs/error-codes.md`](error-codes.md) for the full code table and
> SDK/dashboard mapping guidance.

## Initialization

* `initialize(env, admin)`: Sets the initial admin and grants them the Admin role. Reverts with `AlreadyInitialized` if already initialized.

## Role Management (admin.rs)

* `set_role(env, admin, target, role)`: Assigns a role to `target`. Requires Admin role (`Unauthorized`). Cannot assign the Admin role — use `transfer_admin` instead (`CannotAssignAdminRole`). Emits `RoleAssignedEvent`.
* `remove_role(env, admin, target)`: Revokes the role from `target`. Requires Admin role (`Unauthorized`). Reverts with `NoRoleToRevoke` if target has no role. Emits `RoleRevokedEvent`.
* `get_role_of(env, address)`: Returns the role assigned to `address`, or `Role::None`.
* `transfer_admin(env, admin, candidate)`: Initiates a 2-step admin transfer. Requires Admin role (`Unauthorized`). Emits `AdminTransferInitiatedEvent`.
* `accept_admin(env, candidate)`: Completes a 2-step admin transfer. Reverts with `NoPendingAdminTransfer` if none is in flight, or `NotPendingCandidate` if the caller isn't the recorded candidate. Emits `AdminTransferredEvent`.
* `renounce_admin(env, admin)`: The admin renounces their own role. Requires Admin role (`Unauthorized`). Irreversible. Emits `AdminTransferredEvent`.

Any call above made before `initialize` reverts with `NotInitialized`.

## Emergency Pause (admin.rs)

* `pause(env, caller)`: Pauses the contract. Requires Admin or EmergencyOfficer role (`Unauthorized`). Reverts with `AlreadyPaused` if already paused. Emits `ContractPausedEvent`.
* `unpause(env, caller)`: Unpauses the contract. Requires Admin role only (`Unauthorized`). Reverts with `NotPaused` if not paused. Emits `ContractUnpausedEvent`.
* `is_paused(env)`: Returns whether the contract is currently paused. Always available.
* All state-changing calls listed on this page revert with `ContractPaused` while the contract is paused.

## Compliance (compliance.rs)

* `whitelist_user(env, admin, user)`: Adds `user` to the persistent compliance map. Requires ComplianceOfficer role (or Admin) (`Unauthorized`). Blocked when paused (`ContractPaused`).
* `revoke_whitelist(env, admin, user)`: Removes `user` from the compliance whitelist. Requires ComplianceOfficer role (or Admin) (`Unauthorized`). Blocked when paused (`ContractPaused`).

## Asset Operations (asset.rs)

* `mint_asset(env, admin, to, amount)`: Mints `amount` to `to`. Requires AssetManager role (or Admin) (`Unauthorized`). Reverts with `InvalidAmount` if `amount <= 0`, or `ReceiverNotWhitelisted` if `to` is not whitelisted. Blocked when paused (`ContractPaused`).
* `transfer(env, from, to, amount)`: Moves `amount` between addresses. Requires `from` auth. Reverts with `InvalidAmount` if `amount <= 0`; `SenderNotWhitelisted` or `ReceiverNotWhitelisted` if either party is not whitelisted; `InsufficientBalance` if `from` cannot cover `amount`. Blocked when paused (`ContractPaused`).
* `distribute_yield(env, admin, amount)`: Triggers a dividend yield event for off-chain indexing. Requires AssetManager role (or Admin) (`Unauthorized`). Reverts with `InvalidAmount` if `amount <= 0`. Blocked when paused (`ContractPaused`).

## Read Functions

These functions are always available, even when the contract is paused:

* `get_balance_of(env, address)`: Returns the token balance for an address (defaults to 0).
* `get_total_supply(env)`: Returns the global total supply (defaults to 0).
* `is_whitelisted(env, user)`: Returns whether an address is on the compliance whitelist.
