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
| `role_assigned`              | `RoleAssignedEvent`          | `admin.rs`      | `set_role`                                      | `admin: Address`, `target: Address`, `role: Role` |
| `role_revoked`                | `RoleRevokedEvent`           | `admin.rs`      | `remove_role`                                   | `admin: Address`, `target: Address`, `role: Role` |
| `admin_transfer_initiated`    | `AdminTransferInitiatedEvent`| `admin.rs`      | `transfer_admin`                                | `current_admin: Address`, `candidate: Address` |
| `admin_transferred`           | `AdminTransferredEvent`      | `admin.rs`      | `accept_admin`, `renounce_admin`                | `previous_admin: Address`, `new_admin: Address` (equal to `previous_admin` on renounce) |
| `contract_paused`             | `ContractPausedEvent`        | `admin.rs`      | `pause`                                         | `admin: Address` (the pausing caller — Admin or EmergencyOfficer) |
| `contract_unpaused`           | `ContractUnpausedEvent`      | `admin.rs`      | `unpause`                                       | `admin: Address` |
| `user_whitelisted`            | `UserWhitelistedEvent`       | `compliance.rs` | `whitelist_user`                                | `caller: Address`, `user: Address` |
| `whitelist_revoked`           | `WhitelistRevokedEvent`      | `compliance.rs` | `revoke_whitelist`                              | `caller: Address`, `user: Address` |
| `asset_minted`                | `AssetMintedEvent`           | `asset.rs`      | `mint_asset`                                    | `caller: Address`, `to: Address`, `amount: i128`, `total_supply: i128` |
| `transfer`                    | `TransferEvent`              | `asset.rs`      | `transfer`                                      | `from: Address`, `to: Address`, `amount: i128` |
| `yield_distributed`           | `YieldDistributedEvent`      | `asset.rs`      | `distribute_yield`                              | `caller: Address`, `amount: i128` |

> **Compliance transitions:** the authorisation, blocked (paused), and
> idempotence semantics that determine *when* `user_whitelisted` /
> `whitelist_revoked` are emitted — and the invariant tests asserting that
> rejected transitions emit nothing — are defined in
> [`compliance-status-transitions.md`](compliance-status-transitions.md).

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

- `test_whitelist_user_emits_event`
- `test_revoke_whitelist_emits_event`
- `test_mint_asset_emits_event_with_running_supply`
- `test_transfer_emits_event`
- `test_blocked_transfer_emits_no_event`
- `test_distribute_yield_emits_event`

Add a new compatibility test alongside any new or changed event so a schema
drift fails CI instead of shipping silently to downstream consumers.

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
