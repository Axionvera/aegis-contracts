# Contract Capability Flags

This document describes the read-only capability flags exposed by the Aegis
RWA Contracts. They advertise **which modules are enabled and which protocol
behaviours are supported** by a given deployment, so SDK and dashboard
clients can feature-gate their UI from a single call.

Like [`docs/error-codes.md`](error-codes.md) numeric codes and
[`docs/events.md`](events.md) topics, the capability keys and field names
below are a **stable contract**: match on field/key name, never on struct
declaration order or Rust type layout.

> **Not a permission check.** A capability says the protocol *implements* a
> behaviour, not that *you* may perform it or that it will succeed right now.
> Authorization is still governed by [`docs/admin-roles.md`](admin-roles.md),
> and per-investor eligibility by
> [`docs/investor-eligibility.md`](investor-eligibility.md). Never use a
> capability flag as an access-control decision.

## Why

Before this change, a front-end had no way to ask the contract what it could
do. Clients had three bad options:

1. **Hardcode a feature matrix per deployment** — silently wrong the moment a
   contract is upgraded or a second deployment ships with a different build.
2. **Probe entrypoints and catch the revert** — expensive, noisy, and
   indistinguishable from a genuine authorization or state failure. A missing
   `approve` and an unauthorized `approve` look identical off-chain.
3. **Parse the contract spec XDR** — workable for a bespoke tool, but it only
   reveals that a *function exists*, not whether the behaviour behind it is
   actually implemented. `distribute_yield` exists but settles nothing
   on-chain; a spec dump cannot tell you that.

The capability read helper answers the question directly, in one call, from
the deployment itself.

## Static capability vs. runtime switch

Two kinds of field appear in the response and **must not be conflated**:

| Kind | Fields | Caching |
| --- | --- | --- |
| **Static capability** | every `CapabilityStatus` field, and the `module_enabled` booleans | Fixed for a given contract build. Safe to cache for the lifetime of a deployment. |
| **Runtime switch** | `initialized`, `paused`, `asset_active`, `operations_enabled`, `supply_cap_enforced`, `holding_cap_enforced`, `metadata_configured` | Derived from current ledger state; can change between calls. Re-read; do not cache. |

The distinction matters. `pause.global_pause` is `Supported` **even while the
contract is paused** — the capability exists; it is simply currently active.
The runtime flag `pause.paused` is what tells you operations are halted.
A client that gates on the wrong one will hide its pause banner exactly when
it needs to show it.

## `CapabilityStatus`

Every behaviour flag is a tri-state, not a `bool`. A plain boolean cannot
distinguish "this contract will never do that" from "not built yet, but
tracked" — and front-ends need that distinction to choose between hiding a
control permanently and rendering a "coming soon" affordance.

| Status | Meaning | Recommended client behaviour |
| --- | --- | --- |
| `Supported` | Implemented and callable against this deployment now. | Render the feature normally. |
| `Planned` | Not available yet; a known, documented gap a future version is expected to close. | Render a disabled / "coming soon" control. **Never build a transaction against it.** |
| `Unsupported` | Not available, and not a tracked gap — deliberately out of scope, or impossible under the protocol's design. | Hide the corresponding UI entirely. |

### Unsupported states are explicit

The following are `Unsupported` and, importantly, **why** — so integrators do
not file them as bugs or wait for them:

| Capability | Why it is `Unsupported` |
| --- | --- |
| `minting.burning` | No burn entrypoint exists. Supply is monotonically increasing; a lowered supply cap blocks future mints rather than burning existing units (see [`supply-cap-governance.md`](supply-cap-governance.md)). |

| `compliance.investor_tiers` | `DataKey::Whitelist` is a single boolean carrying no jurisdiction, accreditation tier, or investor-class data. Regime-specific segmentation (e.g. Reg D vs. Reg S) is off-chain only — see [`threat-model.md`](threat-model.md) C-4. |
| `events.transfer_restriction_events` | **Structurally impossible.** Soroban discards events from a reverted invocation, so a blocked transfer can never durably publish one. Watch the granular restriction error codes (`3004`, `4000`, `4001`, `7000`–`7004`) or call `check_transfer_restriction` instead — see [`events.md`](events.md#transfer-restriction-events). |

| `compliance.investor_tiers` | The compliance lifecycle models compliance *state* (`Unknown`/`Pending`/`Approved`/`Revoked`/`Blocked`), not investor *class*. It carries no jurisdiction or accreditation-tier data, so regime-specific segmentation (e.g. Reg D vs. Reg S) is off-chain only — see [`threat-model.md`](threat-model.md) C-4. |
| `events.transfer_restriction_events` | **Structurally impossible.** Soroban discards events from a reverted invocation, so a blocked transfer can never durably publish one. Watch error codes `3004`/`4000`/`4001` instead — see [`events.md`](events.md#transfer-restriction-events). |


`Planned` items — `allowances`, `transfer_from`, `transfer_fees`,
`decimals`, `sep41_token_interface`, `batch_whitelisting`,
`yield_distribution`, and `asset_registered_event` — correspond to the gaps
tracked in
[`dashboard-readiness-review.md`](dashboard-readiness-review.md) and the
`// TODO:` markers in the source.

## API

All three functions are pure reads: **no storage writes, no events, no
authorization required, and they never panic** — including *before*
`initialize` has been called and while the contract is paused. They are safe
to call from a read-only RPC simulation at any time.

### `get_capabilities() -> ContractCapabilities`

Returns the full descriptor.

```rust
pub struct ContractCapabilities {
    pub capability_version: u32,        // schema version of this response
    pub contract_version: String,       // crate version, e.g. "0.1.0"
    pub initialized: bool,              // runtime: has initialize() been called
    pub rbac: CapabilityStatus,
    pub two_step_governance: CapabilityStatus,
    pub sep41_token_interface: CapabilityStatus,
    pub compliance: ComplianceCapabilities,
    pub minting: MintingCapabilities,
    pub transfers: TransferCapabilities,
    pub pause: PauseCapabilities,
    pub metadata: MetadataCapabilities,
    pub events: EventCapabilities,
}
```

#### `compliance`

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `module_enabled` | `bool` | `true` | Compliance module compiled in. |
| `whitelist` | status | `Supported` | `whitelist_user`. |
| `whitelist_revocation` | status | `Supported` | `revoke_whitelist`. |
| `batch_whitelisting` | status | `Planned` | Many addresses per invocation. |
| `investor_tiers` | status | `Unsupported` | Jurisdiction/accreditation tiers. |
| `lifecycle_states` | status | `Supported` | Five-state compliance lifecycle + `get_compliance_status`. See [`compliance-lifecycle.md`](compliance-lifecycle.md). |
| `lifecycle_transitions` | status | `Supported` | Enforced transition matrix on `set_compliance_status`, plus the pre-flight transition reads. |
| `eligibility_reads` | status | `Supported` | `get_investor_eligibility`, `check_transfer_eligibility`. |
| `enforced_on_mint` | `bool` | `true` | Every mint checks the receiver's lifecycle status. |
| `enforced_on_transfer` | `bool` | `true` | Every transfer checks both parties' lifecycle statuses. |

#### `minting`

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `module_enabled` | `bool` | `true` | Minting module compiled in. |
| `minting` | status | `Supported` | `mint_asset`. |
| `burning` | status | `Unsupported` | No burn entrypoint. |
| `supply_cap` | status | `Supported` | Global cap with 2-step governance. |
| `supply_cap_enforced` | `bool` **(runtime)** | `false` | A cap is currently active (`> 0`). |
| `yield_distribution` | status | `Planned` | `distribute_yield` emits an event only; it settles no value on-chain. |

#### `transfers`

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `module_enabled` | `bool` | `true` | Transfer module compiled in. |
| `transfers` | status | `Supported` | `transfer`. |
| `holding_cap` | status | `Supported` | Per-investor cap with 2-step governance. |
| `holding_cap_enforced` | `bool` **(runtime)** | `false` | A holding cap is currently active (`> 0`). |
| `allowances` | status | `Planned` | SEP-41 `approve` / `allowance`. |
| `transfer_from` | status | `Planned` | SEP-41 `transfer_from`. |
| `transfer_fees` | status | `Planned` | Fee deduction on transfer. |
| `transfer_eligibility_check` | status | `Supported` | `check_transfer_eligibility`. |
| `transfer_restriction_reasons` | status | `Supported` | `check_transfer_restriction`, `check_mint_restriction`, `get_restriction_code` — granular blocked-transfer reason codes. See [transfer-restrictions.md](transfer-restrictions.md). |

#### `pause`

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `module_enabled` | `bool` | `true` | Pause module compiled in. |
| `global_pause` | status | `Supported` | `pause` / `unpause`. |
| `paused` | `bool` **(runtime)** | `false` | Contract is currently globally paused. |
| `asset_lifecycle` | status | `Supported` | `set_asset_status`. |
| `asset_active` | `bool` **(runtime)** | `true` | Lifecycle status is `Active`. |
| `operations_enabled` | `bool` **(runtime, derived)** | `true` | `!paused && asset_active`. When `false`, no mint or transfer can succeed for **any** investor. |

`operations_enabled` is a protocol-level switch only. An investor may still
be individually ineligible while it is `true` — use
`get_investor_eligibility` for the per-address answer.

#### `metadata`

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `module_enabled` | `bool` | `true` | Metadata module compiled in. |
| `name_and_symbol` | status | `Supported` | Readable/writable name and ticker. |
| `metadata_uri` | status | `Supported` | Off-chain metadata URI pointer. |
| `decimals` | status | `Planned` | SEP-41 `decimals`. **Do not infer a precision.** |
| `metadata_configured` | `bool` **(runtime)** | `false` | A non-empty name *and* symbol have been set. |
| `lifecycle_restricted` | `bool` | `true` | Updates are blocked in `Retired`/`Blocked`. |

#### `events`

Mirrors the topics in [`events.md`](events.md). `compliance_events`,
`compliance_lifecycle_events`, `minting_events`, `transfer_events`,
`admin_events`, `governance_events`, and `asset_lifecycle_events` are all
`Supported`;
`transfer_restriction_events` is `Unsupported` and `asset_registered_event`
is `Planned`.

### `supports_capability(capability: Symbol) -> CapabilityStatus`

Resolves a single key, derived from the same descriptor so the two can never
disagree.

**Unknown keys resolve to `Unsupported` rather than reverting.** This is
deliberate: a newer SDK probing an older deployment fails safe and simply
hides the feature, instead of the call trapping and the dashboard rendering
an error.

Registry (also returned by `get_capability_keys()`):

| Key | Resolves to |
| --- | --- |
| `rbac` | `rbac` |
| `two_step_governance` | `two_step_governance` |
| `sep41` | `sep41_token_interface` |
| `compliance` | `compliance.module_enabled` |
| `whitelist` | `compliance.whitelist` |
| `whitelist_revocation` | `compliance.whitelist_revocation` |
| `batch_whitelisting` | `compliance.batch_whitelisting` |
| `investor_tiers` | `compliance.investor_tiers` |
| `compliance_lifecycle` | `compliance.lifecycle_states` |
| `compliance_transitions` | `compliance.lifecycle_transitions` |
| `eligibility_reads` | `compliance.eligibility_reads` |
| `minting` | `minting.minting` |
| `burning` | `minting.burning` |
| `supply_cap` | `minting.supply_cap` |
| `yield_distribution` | `minting.yield_distribution` |
| `transfers` | `transfers.transfers` |
| `holding_cap` | `transfers.holding_cap` |
| `allowances` | `transfers.allowances` |
| `transfer_from` | `transfers.transfer_from` |
| `transfer_fees` | `transfers.transfer_fees` |
| `transfer_eligibility` | `transfers.transfer_eligibility_check` |
| `transfer_restriction_reasons` | `transfers.transfer_restriction_reasons` |
| `pause` | `pause.global_pause` |
| `asset_lifecycle` | `pause.asset_lifecycle` |
| `metadata` | `metadata.name_and_symbol` |
| `metadata_uri` | `metadata.metadata_uri` |
| `decimals` | `metadata.decimals` |
| `events` | `events.module_enabled` |
| `compliance_lifecycle_events` | `events.compliance_lifecycle_events` |
| `transfer_restriction_events` | `events.transfer_restriction_events` |
| `asset_registered_event` | `events.asset_registered_event` |

### `get_capability_keys() -> Vec<Symbol>`

Returns every key this contract version understands, so a client can
enumerate the registry rather than hardcode it — and detect at runtime that a
deployment is older or newer than the keys it knows about. Order is stable
within a schema version.

## Versioning

`capability_version` is the schema version of the response

(`CAPABILITY_SCHEMA_VERSION`, currently `2`); `contract_version` is the

(`CAPABILITY_SCHEMA_VERSION`, currently `2` — bumped when the compliance
lifecycle fields and keys were added); `contract_version` is the

deployed crate's semantic version.

Bump `capability_version` whenever a field is **added** to any capability
struct or a key is added to the registry, so an SDK pinned to an older schema
can detect that the deployment may advertise capabilities it does not know
about.

Fields and keys are **append-only**. Never remove or repurpose an existing
one — downstream clients may have it hardcoded. Flipping a status from
`Planned` to `Supported` when a feature actually ships is the expected
lifecycle and does *not* require a schema bump; adding the field in the first
place does.

Clients should treat an **unrecognised** `capability_version` as "newer than
me": read the fields they know, ignore the rest, and fall back to
`Unsupported` for anything absent.

## SDK and dashboard usage

- **Feature-gate navigation at load.** Call `get_capabilities()` once on app
  init and cache the *static* fields. Hide the "Approvals" tab while
  `transfers.allowances != Supported`; hide any burn control while
  `minting.burning == Unsupported`.
- **Render `Planned` differently from `Unsupported`.** `Planned` → a disabled
  control with a "coming soon" tooltip. `Unsupported` → no control at all.
  Never build a transaction against a non-`Supported` capability, even if the
  entrypoint appears in the contract spec.
- **Do not cache runtime switches.** Re-read `paused`, `operations_enabled`,
  `*_enforced`, and `metadata_configured` on each view. Use
  `pause.operations_enabled` for a global "trading halted" banner, then
  `get_investor_eligibility` for the per-investor reason.
- **Gate cap indicators on the runtime flag.** Only show a "X of Y capacity
  used" meter when `transfers.holding_cap_enforced` is `true`; otherwise
  `remaining_capacity` is `None` and there is no ceiling to render.
- **Fall back on unconfigured metadata.** When `metadata_configured` is
  `false`, render a placeholder rather than blank strings, and never infer a
  decimal precision while `metadata.decimals` is `Planned`.
- **Show a setup state when `initialized` is `false`.** Every privileged
  entrypoint will revert with `NotInitialized` (2000) until it flips.
- **Probing is safe.** `supports_capability` with an unknown key returns
  `Unsupported`, so a client can safely ask about features that may not exist
  yet without special-casing the error path.
- All three are ordinary read calls — invoke them like `get_balance_of` /
  `is_whitelisted` via `soroban contract invoke` (read-only, no signing) or
  the generated SDK client's simulate-only path.

## Compatibility

- **Purely additive.** No existing function, error code, event, or storage
  key changed. `asset::get_asset_status_internal` was widened from private to
  `pub` so the capability module reads lifecycle state through the same
  helper `mint_asset`/`transfer` use — behaviour is unchanged, and there is
  now one source of truth rather than a duplicated default.
- **No new storage keys and no new error codes.** Every read falls back to
  the same safe default its owning module uses, which is why the helper
  cannot panic on an uninitialized contract.
- **Not a state-changing call**, so it is exempt from the pause guard by
  design — consistent with the other read helpers documented in
  [`contract-spec.md`](contract-spec.md#read-functions).
- Tests covering the default capability state (before and after
  `initialize`), the no-mutation guarantee, paused and lifecycle states,
  active caps, metadata configuration, explicit unsupported/planned states,
  unknown-key fail-safe behaviour, and registry/descriptor agreement live in
  [`src/test.rs`](../src/test.rs).
