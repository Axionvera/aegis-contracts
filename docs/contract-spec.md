# Contract API Specification

> Every revert below is a standardized `Error` code, not a message string.
> See [`docs/error-codes.md`](error-codes.md) for the full code table and
> SDK/dashboard mapping guidance. See [`docs/events.md`](events.md) for the
> full event topic/payload schema and compatibility guarantees.

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

* `whitelist_user(env, admin, user)`: Adds `user` to the persistent compliance map. Requires ComplianceOfficer role (or Admin) (`Unauthorized`). Blocked when paused (`ContractPaused`). Emits `UserWhitelistedEvent`.
* `revoke_whitelist(env, admin, user)`: Removes `user` from the compliance whitelist. Requires ComplianceOfficer role (or Admin) (`Unauthorized`). Blocked when paused (`ContractPaused`). Emits `WhitelistRevokedEvent`.

## Asset Operations (asset.rs)

* `mint_asset(env, admin, to, amount)`: Mints `amount` to `to`. Requires AssetManager role (or Admin) (`Unauthorized`). Reverts with `InvalidAmount` if `amount <= 0`; `SupplyCapExceeded` (`5002`) if minting would exceed the active global supply cap; `ReceiverNotWhitelisted` if `to` is not whitelisted. Blocked when paused (`ContractPaused`). Emits `AssetMintedEvent` (includes the running `total_supply`).
* `transfer(env, from, to, amount)`: Moves `amount` between addresses. Requires `from` auth. Reverts with `InvalidAmount` if `amount <= 0`; `SenderNotWhitelisted` or `ReceiverNotWhitelisted` if either party is not whitelisted; `InsufficientBalance` (`5001`) if `from` cannot cover `amount`; `HoldingCapExceeded` (`5003`) if the receiver's balance would exceed the active holding cap. Blocked when paused (`ContractPaused`). Emits `TransferEvent` on success only — a compliance-blocked transfer reverts and emits nothing; see [`docs/events.md`](events.md#transfer-restriction-events).
* `distribute_yield(env, admin, amount)`: Triggers a dividend yield event for off-chain indexing. Requires AssetManager role (or Admin) (`Unauthorized`). Reverts with `InvalidAmount` if `amount <= 0`. Blocked when paused (`ContractPaused`). Emits `YieldDistributedEvent`.

## Supply Cap Governance (supply_cap.rs)

* `get_supply_cap(env)`: Returns the active global supply cap (`0` = unbounded).
* `get_pending_supply_cap(env)`: Returns the pending proposed cap (`None` if none).
* `propose_supply_cap(env, admin, proposed_cap)`: Initiates a 2-step cap amendment (`supply_cap_proposed` event). Only admin; blocked when paused. Rejects negative or no-op proposals.
* `accept_supply_cap(env, admin)`: Activates the pending cap (`supply_cap_amended` event). Only admin; blocked when paused.
* `cancel_supply_cap_proposal(env, admin)`: Discards a pending proposal. Only admin; blocked when paused.
* Enforcement (`enforce_supply_cap`): `mint_asset` calls this before increasing total supply. Reverts with `SupplyCapExceeded` (`5002`) when `total_supply + amount > cap`. A cap of `0` means no cap enforced.

## Holding Cap Governance (holding.rs)

* `get_holding_cap(env)`: Returns the active per-investor holding cap (`0` = unrestricted).
* `get_pending_holding_cap(env)`: Returns the pending proposed cap (`None` if none).
* `propose_holding_cap(env, admin, proposed_cap)`: Initiates a 2-step cap amendment (`holding_cap_proposed` event). Only admin; blocked when paused. Rejects negative or no-op proposals.
* `accept_holding_cap(env, admin)`: Activates the pending cap (`holding_cap_amended` event). Only admin; blocked when paused.
* `cancel_holding_cap_proposal(env, admin)`: Discards a pending proposal. Only admin; blocked when paused.
* Enforcement (`enforce_holding_cap`): `mint_asset` and `transfer` call this before crediting the receiver. Reverts with `HoldingCapExceeded` (`5003`) when `balance + incoming > cap`. A cap of `0` means no restriction.

## Read Functions

These functions are always available, even when the contract is paused:

* `get_balance_of(env, address)`: Returns the token balance for an address (defaults to 0).
* `get_total_supply(env)`: Returns the global total supply (defaults to 0).
* `is_whitelisted(env, user)`: Returns whether an address is on the compliance whitelist.

## Investor Eligibility (eligibility.rs)

Pure reads that compose the checks above into single-call answers for SDK and
dashboard consumers. Never mutate state; always available, even when paused.
See [`docs/investor-eligibility.md`](investor-eligibility.md) for field
semantics and SDK usage guidance.

* `get_investor_eligibility(env, investor)`: Returns an `InvestorEligibility` struct with the investor's whitelist status, the contract's pause state, current balance, active holding cap, remaining holding-cap capacity, and derived `can_send`/`can_receive` flags.
* `check_transfer_eligibility(env, from, to, amount)`: Returns `true` if a transfer of `amount` from `from` to `to` would currently pass every check `transfer()` performs (pause, whitelist on both sides, holding cap, sender balance).
