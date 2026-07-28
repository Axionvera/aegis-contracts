# Contract Event Schema

This document defines the structured events emitted by the Aegis RWA
Protocol contracts, so SDKs, dashboards, indexers, and compliance tooling
have a stable, documented interface to build against instead of reverse
engineering event shapes from contract source.

## Why

Events are the primary way off-chain systems observe on-chain state changes
without polling storage. Treat the topics and payload field names below as a
stable contract, the same way [`docs/error-codes.md`](error-codes.md) treats
numeric error codes as a stable contract: **match on topic string and field
name, never on struct declaration order or Rust type layout.**

## Conventions

- Every event is published via `env.events().publish((topic,), PayloadStruct)`
  with a single string topic (see each table row below).
- Payload structs are `#[contracttype]` values — SDKs generated from the
  contract spec (`soroban contract bindings`) get typed accessors for free.
- The `caller` field (where present) is the address that authorized the
  action, which may be the Admin or a scoped role holder (ComplianceOfficer,
  AssetManager, EmergencyOfficer) — see [`docs/admin-roles.md`](admin-roles.md).
  It is **not** always the global Admin.
- **No sensitive or off-chain data is ever emitted.** Payloads are limited to
  on-chain addresses, amounts, and role enum values already visible via the
  contract's public read functions. No KYC documents, personal identifiers,
  or off-chain compliance-provider data are included in any event.
- Soroban discards all events (and all state changes) from a reverted
  invocation. An event only exists off-chain if the transaction that
  published it actually committed.

## Event reference

| Topic                       | Payload struct              | Module         | Emitted by                                    | Fields |
|------------------------------|------------------------------|----------------|------------------------------------------------|--------|
| `contract_initialized`        | `ContractInitializedEvent`   | `lib.rs`        | `initialize`                                    | `admin: Address` |
| `role_assigned`              | `RoleAssignedEvent`          | `admin.rs`      | `set_role`                                      | `admin: Address`, `target: Address`, `role: Role` |
| `role_revoked`                | `RoleRevokedEvent`           | `admin.rs`      | `remove_role`                                   | `admin: Address`, `target: Address`, `role: Role` |
| `admin_transfer_initiated`    | `AdminTransferInitiatedEvent`| `admin.rs`      | `transfer_admin`                                | `current_admin: Address`, `candidate: Address` |
| `admin_transferred`           | `AdminTransferredEvent`      | `admin.rs`      | `accept_admin`, `renounce_admin`                | `previous_admin: Address`, `new_admin: Address` (equal to `previous_admin` on renounce) |
| `admin_renounced`             | `AdminTransferredEvent`      | `admin.rs`      | `renounce_admin`                                | `previous_admin: Address`, `new_admin: Address` (both equal to the renouncing admin) |
| `contract_paused`             | `ContractPausedEvent`        | `admin.rs`      | `pause`                                         | `admin: Address` (the pausing caller — Admin or EmergencyOfficer) |
| `contract_unpaused`           | `ContractUnpausedEvent`      | `admin.rs`      | `unpause`                                       | `admin: Address` |
| `user_whitelisted`            | `UserWhitelistedEvent`       | `compliance.rs` | `whitelist_user`                                | `caller: Address`, `user: Address` |
| `whitelist_revoked`           | `WhitelistRevokedEvent`      | `compliance.rs` | `revoke_whitelist`                              | `caller: Address`, `user: Address` |
| `asset_minted`                | `AssetMintedEvent`           | `asset.rs`      | `mint_asset`                                    | `caller: Address`, `to: Address`, `amount: i128`, `total_supply: i128` |
| `transfer`                    | `TransferEvent`              | `asset.rs`      | `transfer`                                      | `from: Address`, `to: Address`, `amount: i128` |
| `yield_distributed`           | `YieldDistributedEvent`      | `asset.rs`      | `distribute_yield`                              | `caller: Address`, `amount: i128` |
| `asset_status_changed`        | `AssetStatusChangedEvent`    | `asset.rs`      | `set_asset_status`                              | `caller: Address`, `previous_status: AssetStatus`, `new_status: AssetStatus` |
| `asset_metadata_updated`      | `AssetMetadataUpdatedEvent`  | `asset.rs`      | `update_asset_metadata`                         | `caller: Address`, `name: String`, `symbol: String`, `uri: String` |
| `supply_cap_proposed`         | `SupplyCapProposedEvent`     | `supply_cap.rs` | `propose_supply_cap`                            | `admin: Address`, `current_cap: i128`, `proposed_cap: i128` |
| `supply_cap_amended`          | `SupplyCapAmendedEvent`      | `supply_cap.rs` | `accept_supply_cap`                             | `admin: Address`, `previous_cap: i128`, `new_cap: i128` |
| `holding_cap_proposed`        | `HoldingCapProposedEvent`    | `holding.rs`    | `propose_holding_cap`                           | `admin: Address`, `current_cap: i128`, `proposed_cap: i128` |
| `holding_cap_amended`         | `HoldingCapAmendedEvent`     | `holding.rs`    | `accept_holding_cap`                            | `admin: Address`, `previous_cap: i128`, `new_cap: i128` |

## Scope notes

### Asset registration vs. minting

The issuance model in this contract version is a plain `i128` balance per
address (`get_balance_of`), not a per-asset entity with its own identity or
metadata. There is currently no separate "register an asset" step distinct
from minting units to a holder — the `6000` error range in
[`docs/error-codes.md`](error-codes.md) is explicitly reserved for a future
asset-metadata module (name, symbol, decimals, schema validation), and a
future `asset_registered` event would be introduced alongside it.

Until then, `AssetMintedEvent` is the canonical event for both issuance and,
for a recipient's first mint, their effective registration as an asset
holder. Downstream consumers building an audit trail today should treat
`asset_minted` as the registration signal; this document will be updated
with a distinct `asset_registered` event if/when a metadata module ships.

### Transfer restriction events

The issue asks that "transfer restriction events are considered where
appropriate." They were considered and deliberately **not** implemented as
a separate event, because Soroban rolls back all events published during an
invocation that ultimately reverts — the same guarantee that rolls back
storage writes. A `transfer` call blocked by compliance never reaches a
point where it could durably publish a "transfer restricted" event; any such
event would necessarily also be dropped, making it indistinguishable from
never having existed.

Instead, the restriction is durably observable via the standardized revert
codes already defined in [`docs/error-codes.md`](error-codes.md):

- `4000 SenderNotWhitelisted`
- `4001 ReceiverNotWhitelisted`
- `3004 ContractPaused` (blocks all transfers while paused)

SDKs and indexers that need to record restricted-transfer attempts for audit
purposes should watch for these error codes on failed `transfer`/`mint_asset`
simulations or failed transaction results, not for an event.

This is advertised on-chain: `get_capabilities()` reports
`events.transfer_restriction_events` as `Unsupported` (not `Planned`), so a
client can tell the difference between "not built yet" and "structurally
impossible, stop waiting for it". See [`capabilities.md`](capabilities.md).

## Compatibility tests

Every event above has a corresponding test in [`src/test.rs`](../src/test.rs)
asserting its exact topic and payload shape using
`soroban_sdk::testutils::Events`:

**Initialization & Role events:**
- `test_initialize_emits_event`
- `test_set_role_emits_event`
- `test_remove_role_emits_event`

**Admin transfer events:**
- `test_transfer_admin_emits_event`
- `test_accept_admin_emits_event`
- `test_renounce_admin_emits_event`

**Pause events:**
- `test_pause_emits_event`
- `test_unpause_emits_event`

**Compliance events:**
- `test_whitelist_user_emits_event`
- `test_revoke_whitelist_emits_event`

**Asset & transfer events:**
- `test_mint_asset_emits_event_with_running_supply`
- `test_transfer_emits_event`
- `test_blocked_transfer_emits_no_event`
- `test_distribute_yield_emits_event`

Add a new compatibility test alongside any new or changed event so a schema
drift fails CI instead of shipping silently to downstream consumers.

## SDK / Dashboard Compatibility Notes

### Event Consumer Guidance

SDKs and dashboards consuming these events should observe the following
guidelines to maintain forward and backward compatibility:

1. **Match on topic string, not struct layout.** The topic tuple (e.g.
   `("role_assigned",)`) is the stable identifier. Payload field names are
   stable within a schema version; new fields may be appended but existing
   fields will never be removed or renamed.

2. **Role events (`role_assigned`, `role_revoked`)** carry the `Role` enum
   value. The set of roles (`None`, `Admin`, `ComplianceOfficer`,
   `AssetManager`, `EmergencyOfficer`) may grow in future versions. Consumers
   should treat unknown role values as a distinct category rather than
   rejecting the event.

3. **Admin lifecycle events** (`contract_initialized`,
   `admin_transfer_initiated`, `admin_transferred`, `admin_renounced`)
   together form a complete audit trail of admin changes. On `admin_renounced`,
   both `previous_admin` and `new_admin` are equal to the renouncing address;
   consumers should render this distinctly (e.g. "Admin renounced") rather
   than as a transfer.

4. **`contract_initialized`** is emitted exactly once, at deployment time.
   Dashboards building an admin-history timeline should use this as the
   genesis event.

5. **Pause events** (`contract_paused`, `contract_unpaused`) are emitted by
   the admin or EmergencyOfficer. The `admin` field is the *pausing caller*,
   which may not be the supreme Admin — see
   [`admin-roles.md`](admin-roles.md) for the full authorization matrix.

6. **Governance events** (`supply_cap_proposed`, `supply_cap_amended`,
   `holding_cap_proposed`, `holding_cap_amended`) follow a 2-step flow.
   Indexers should correlate a `*_proposed` event with the subsequent
   `*_amended` event via the admin address and cap value; proposals that are
   cancelled emit no event (cancellation is observable only via the
   `get_pending_supply_cap` / `get_pending_holding_cap` read helpers
   returning `None`).

7. **Asset lifecycle events** (`asset_status_changed`,
   `asset_metadata_updated`) carry the full before/after state so consumers
   need not maintain their own copy of the asset status to render a
   transition.

8. **No sensitive data is emitted.** All payloads contain only on-chain
   addresses, amounts, and role/status enum values already readable via the
   contract's public read functions. No KYC documents, personal identifiers,
   or off-chain compliance-provider data appear in any event.

### Capability Discovery

The `get_capabilities()` read helper reports event support at runtime:

| Capability key                  | Status when implemented |
|----------------------------------|------------------------|
| `events`                         | `Supported` when the event module is compiled in |
| `events.compliance_events`       | `Supported` — `user_whitelisted`, `whitelist_revoked` |
| `events.minting_events`          | `Supported` — `asset_minted`, `yield_distributed` |
| `events.transfer_events`         | `Supported` — `transfer` |
| `events.admin_events`            | `Supported` — `role_assigned`, `role_revoked`, `admin_transfer_initiated`, `admin_transferred`, `admin_renounced`, `contract_paused`, `contract_unpaused`, `contract_initialized` |
| `events.governance_events`       | `Supported` — `supply_cap_proposed`, `supply_cap_amended`, `holding_cap_proposed`, `holding_cap_amended` |
| `events.asset_lifecycle_events`  | `Supported` — `asset_status_changed`, `asset_metadata_updated` |
| `events.transfer_restriction_events` | `Unsupported` — structurally impossible under Soroban revert semantics |
| `events.asset_registered_event`  | `Planned` — tracked gap for a future asset-registration module |

Clients can call `supports_capability(Symbol::new("events"))` to check
whether the deployment emits structured events at all, or query individual
event-group keys to feature-gate UI rendering.

## Adding a new event

- Pick a topic name that is a short, `snake_case`, present/past-tense verb
  phrase describing what happened (`asset_minted`, not `mint` or `MintEvent`).
- Add the payload struct next to the function that emits it (see
  `admin.rs`, `compliance.rs`, `asset.rs` for placement conventions), not in
  a separate catch-all events file — keeping the struct next to its emitter
  keeps the two from drifting out of sync.
- Never remove or repurpose an existing topic or field — downstream indexers
  may have it hardcoded. Add new fields as needed for a new use case, but
  treat existing fields as append-only/stable.
- Update the table above and add a compatibility test in `src/test.rs`.
