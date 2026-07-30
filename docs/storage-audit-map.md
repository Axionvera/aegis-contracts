# Contract Storage Layout and Migration Guide

This document is the storage compatibility reference for the Aegis contract.
It describes the layout implemented by `DataKey` in `src/lib.rs`, the value
models stored under those keys, and the review and migration work required when
the layout changes.

It is a snapshot of the current source, not a substitute for inspecting a
release's exact WASM and source revision. A pull request that changes storage
must update this document in the same change.

## Storage model

Aegis uses two Soroban storage classes:

- **Instance storage** holds singleton contract configuration and aggregate
  state. It shares the contract instance's lifetime.
- **Persistent storage** holds address-indexed records. Each entry has its own
  lifetime.

The contract does not currently use temporary storage. It also does not
explicitly extend storage TTLs. Operators must therefore account for the
network's storage-expiration and archival behavior; the Rust defaults described
below apply only when a key is absent and are not a substitute for restoring
expired state.

All keys are values of the `#[contracttype]` enum `DataKey`. The enum and every
stored `#[contracttype]` value are persisted encodings. Renaming, removing, or
changing the shape of a key variant can make existing entries unreachable.
Changing a stored value's type or changing enum/struct variants or fields can
make old values decode incorrectly or fail to decode. Treat these as storage
schema changes even when Rust compilation succeeds.

## Complete key inventory

### Instance storage

| Key | Stored value | Absent-key behavior | Writers and lifecycle |
|---|---|---|---|
| `Admin` | `Address` | `initialize` may run; admin-required reads fail with `NotInitialized` | Set by `initialize`, replaced by `accept_admin`, removed permanently by `renounce_admin` |
| `AdminCandidate` | `Address` | No pending admin transfer | Set/overwritten by `transfer_admin`, removed by `accept_admin` |
| `TotalSupply` | `i128` | Reads as `0` | Increased by successful `mint_asset`; never decreased because there is no burn operation |
| `Paused` | `bool` | Reads as `false` | Set to `true` by `pause` and `false` by `unpause` |
| `SupplyCap` | `i128` | Reads as `0` (unbounded) | Set by `accept_supply_cap`; values are non-negative |
| `SupplyCapCandidate` | `i128` | No pending proposal | Set/overwritten by `propose_supply_cap`; removed by accept or cancel |
| `HoldingCap` | `i128` | Reads as `0` (unrestricted) | Set by `accept_holding_cap`; values are non-negative |
| `HoldingCapCandidate` | `i128` | No pending proposal | Set/overwritten by `propose_holding_cap`; removed by accept or cancel |
| `AssetStatus` | `AssetStatus` | Reads as `Draft` | Set by a valid `set_asset_status` transition |
| `AssetName` | Soroban `String` | Reads as `""` | Set by `update_asset_metadata` |
| `AssetSymbol` | Soroban `String` | Reads as `""` | Set by `update_asset_metadata` |
| `AssetMetadataUri` | Soroban `String` | Reads as `""` | Set by `update_asset_metadata` |
| `IssuerSeparationPolicy` | `IssuerSeparationPolicy` | Reads as the permissive policy documented below | Replaced atomically by `set_issuer_separation_policy` |
| `ProtocolConfig` | `ProtocolConfig` | Reads as `{ min_transfer_amount: 0, max_batch_size: 100 }` | Set by `accept_config` |
| `ProtocolConfigCandidate` | `ProtocolConfig` | No pending proposal | Set/overwritten by `propose_config`; removed by accept or cancel |

### Persistent storage

| Key | Stored value | Absent-key behavior | Writers and lifecycle |
|---|---|---|---|
| `Role(Address)` | `Role` | Reads as `Role::None` | Initial admin is set by `initialize`; changed by role management and admin transfer. Revocation stores `None` rather than removing the key |
| `Whitelist(Address)` | `bool` | Effectively `false` | Compatibility mirror only: set to `true` when status becomes `Approved`, removed for every other status. Contract reads derive approval from `ComplianceStatus` |
| `ComplianceStatus(Address)` | `ComplianceStatus` | Reads as `Unknown` (fail closed) | Written on each valid compliance transition and never reset to `Unknown` or removed |
| `Balance(Address)` | `i128` | Reads as `0` | Increased by mint, debited/credited by transfer; zero balances remain stored |
| `ComplianceApprover(Address)` | `Address` | Reads as `None` | Overwritten whenever the address transitions into `Approved`; deliberately retained when approval is later revoked or blocked |

`Address` in a key is part of the key, so each address has an independent
entry. There is no on-chain address list or storage iteration API in this
contract. A migration that must touch every role, compliance record, balance,
or approver record cannot discover those addresses from storage alone.
Historical events or a separately maintained index are required.

## Stored data models

The definitions in source remain authoritative. These summaries capture the
fields and compatibility-sensitive semantics needed for storage review.

### `Role`

`None`, `Admin`, `ComplianceOfficer`, `AssetManager`, and
`EmergencyOfficer`. `Admin` is assigned only during initialization or accepted
admin transfer. A stored `None` and an absent role currently produce the same
read result, although their raw storage presence differs.

### `ComplianceStatus`

`Unknown`, `Pending`, `Approved`, `Revoked`, and `Blocked`. Absence is
`Unknown`; `Unknown` is never written as a target. Only `Approved` permits
receiving or sending; moving to another state freezes rather than deletes an
existing balance. `Whitelist(Address)` is a legacy derived mirror, not the
source of truth.

### `AssetStatus`

`Draft`, `Active`, `Paused`, `Retired`, and `Blocked`. Absence is `Draft`.
Only `Active` permits minting and transfers. This model is separate from the
contract-wide `Paused` boolean. `Retired` is terminal.

### `ProtocolConfig`

| Field | Type | Constraint / default |
|---|---|---|
| `min_transfer_amount` | `i128` | Must be non-negative; absent config defaults to `0` |
| `max_batch_size` | `u32` | Must be non-zero; absent config defaults to `100` |

Adding, removing, renaming, or changing the type of either field changes the
encoded stored value and needs a migration or a new versioned key.

### `IssuerSeparationPolicy`

| Field | Type | Absent-policy default |
|---|---|---|
| `enforced` | `bool` | `false` |
| `allow_dual_duty_issuance` | `bool` | `true` |
| `allow_self_issuance` | `bool` | `true` |
| `require_independent_approver` | `bool` | `false` |

The all-permissive absent value preserves behavior for deployments created
before the policy key existed. A newly added policy field will not
automatically receive a Rust default when decoding an already stored policy.

## Cross-key invariants

These relationships must remain true before and after a migration:

- The active admin's `Role(Address)` is `Admin`. Accepted transfer changes the
  old role to `None`, changes the new role to `Admin`, updates `Admin`, and
  removes `AdminCandidate` atomically. Renunciation intentionally leaves no
  active admin.
- `Whitelist(address)` is present with `true` if and only if
  `ComplianceStatus(address)` is `Approved`. New code must never make the
  legacy mirror authoritative.
- Every balance is non-negative and the sum of all balances equals
  `TotalSupply`. Transfers conserve supply; minting increases a balance and
  supply by the same amount.
- Active and candidate governance keys are distinct. Accepting a cap or config
  copies the candidate to the active key and removes the candidate.
- A cap of `0` means disabled. Lowering a cap below existing supply or balance
  does not rewrite existing state; it prevents later credits that would exceed
  the cap.
- `ComplianceApprover(address)` records the most recent transition into
  `Approved`. Leaving `Approved` must not clear it.

Soroban transaction rollback makes a failed invocation atomic: storage writes
and events from the failed invocation are discarded. Migration code must
preserve that property and must not deliberately split coupled invariants
across transactions without a documented safe intermediate state.

## Upgrade and migration assumptions

An in-place contract code upgrade and deploying a new contract are different
operations:

- An **in-place WASM upgrade at the same contract address** retains that
  contract's storage, subject to ledger lifetime/archival rules. The new WASM
  must still understand every retained key and value.
- A **new deployment at a new contract address** has a different storage
  namespace. Neither instance nor persistent entries automatically follow it.
  State must be exported, verified, and explicitly imported, and integrations
  must move to the new contract ID.

The current Aegis public interface exposes neither a WASM-upgrade entrypoint
nor a storage migration entrypoint, and there is no stored schema-version key.
Consequently, documenting a proposed schema is not enough to make it
deployable. Before changing an incompatible layout, the PR must define the
authorized upgrade mechanism and one of these strategies:

1. **Backward-compatible read:** keep the existing key and encoding; add only
   behavior that can interpret it.
2. **Versioned key with lazy migration:** add a new `DataKey` variant, read the
   new entry first, fall back to the old entry, validate, then write the new
   entry. Define whether and when the old entry is removed.
3. **Explicit bounded migration:** add an authenticated, replay-safe operation
   that converts a known set of entries. Address-indexed migrations must accept
   bounded batches because the contract cannot enumerate all addresses.
4. **New deployment and state import:** define the snapshot source, contract-ID
   cutover, import authorization, reconciliation, and rollback plan.

Never reuse an old key for unrelated data, and do not rely on
`unwrap_or(...)` to handle an incompatible encoded value: it handles an absent
key, not a value that cannot be decoded as the requested type.

### Migration plan requirements

Every incompatible storage change must document:

- the source and destination schema, including exact key and value types;
- supported starting versions and how the deployed version is detected;
- authorization and pause requirements;
- how all affected addresses are discovered;
- batching limits, retry/idempotency behavior, and partial-progress tracking;
- invariant checks before, during, and after migration;
- expected events or other audit evidence;
- TTL/archival restoration assumptions;
- integration impact and, for a new deployment, contract-ID cutover;
- rollback or forward-fix procedure and the point after which rollback is no
  longer safe.

Migration tests must begin with state encoded in the old layout. Tests that
only write and read the new model do not demonstrate compatibility.

## Storage-changing PR risk guide

| Change | Typical risk | Required response |
|---|---|---|
| Add a key with an absent-key default | Lower, but default behavior may alter security or economics | Document the key/default and test both absent and present cases |
| Add a key without a default | Uninitialized reads can fail | Define initialization/backfill ordering and test a pre-change deployment |
| Rename/remove/change a `DataKey` variant | Existing state can become unreachable | Keep a legacy reader or provide an explicit migration |
| Change a stored value type or struct/enum shape | Existing values may no longer decode | Use a versioned representation and old-state migration tests |
| Change an absent-key default | Existing deployments change behavior without a write | Treat as a behavioral migration and review all pre-existing states |
| Move between instance and persistent storage | Lifetime and lookup semantics change | Copy and verify data; define expiration handling and cleanup |
| Add a new address-indexed record | Indexers cannot enumerate it from contract storage | Define discovery/events and any backfill source |
| Change `Balance` or `TotalSupply` | Can violate token conservation | Reconcile every balance against supply and independently review arithmetic |
| Change compliance or role state | Can authorize an ineligible address or remove governance | Security review, negative tests, and post-migration authorization audit |
| Replace the contract instead of upgrading in place | All state remains under the old contract ID | Full export/import and integration cutover plan |

## Mandatory reviewer checklist

For any PR that adds, removes, renames, reorders, repurposes, or changes the
type/default/storage class of persisted data, authors and reviewers must
complete this checklist:

- [ ] Every affected `DataKey`, value model, storage class, default, reader,
      writer, and removal path is identified.
- [ ] This document and relevant module documentation are updated.
- [ ] Compatibility is classified as backward-compatible, lazy-migrated,
      explicitly migrated, or new-deployment-only, with justification.
- [ ] Tests construct the previous deployed layout and prove reads or migration
      under the new code.
- [ ] Absent, legacy, malformed/incompatible, partially migrated, and repeated
      migration cases are considered.
- [ ] Authorization, pause behavior, atomicity, batching, idempotency, and
      storage lifetime are reviewed.
- [ ] Cross-key invariants are asserted after success and remain intact after
      every tested failure.
- [ ] Address discovery and off-chain indexer dependencies are documented.
- [ ] SDK, dashboard, indexer, event, and contract-ID impacts are documented.
- [ ] A rollback or forward-fix plan and operational verification steps exist.
- [ ] A second reviewer explicitly approves changes to balances, total supply,
      admin/roles, or compliance state.

If a checklist item is not applicable, the PR must say why; an unchecked item
without an explanation is not review-ready.

## Source map

- Key enum and basic reads: `src/lib.rs`
- Admin, roles, and contract pause: `src/admin.rs`
- Balances, supply, and metadata: `src/asset.rs`
- Compliance status and whitelist mirror: `src/compliance.rs`
- Asset lifecycle: `src/lifecycle.rs`
- Supply and holding caps: `src/supply_cap.rs`, `src/holding.rs`
- Protocol configuration: `src/config.rs`
- Issuer policy and approver records: `src/issuer.rs`
