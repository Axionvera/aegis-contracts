# Public Interface Compatibility Checks

This document describes `check_interface_compatibility`, a read-only entrypoint
that lets an SDK or dashboard client verify its required capabilities against
a specific Aegis deployment **before** it starts building transactions
against it.

It builds directly on [`docs/capabilities.md`](capabilities.md) — read that
first if you are not already familiar with `CapabilityStatus`,
`get_capabilities`, and the append-only versioning rules. This document only
covers the compatibility check itself.

> **Not a permission or compliance check.** Like the capability flags it is
> built on, this only reports what the *protocol* implements. It is not
> legal, financial, or compliance advice, and it never determines whether a
> specific caller is authorized to do anything — see
> [`docs/admin-roles.md`](admin-roles.md) and
> [`docs/investor-eligibility.md`](investor-eligibility.md) for that.

## Why

`get_capabilities` and `supports_capability` already let a client *ask* what
a deployment supports. What they don't do is give a client a single,
actionable **yes/no plus a reason** for "can I safely integrate with this
deployment at all?" Without that, every integrator re-implements the same
comparison logic — or skips it, and discovers a gap only when a transaction
it assumed would succeed reverts. That is a worse failure mode for
RWA/compliance tooling than for a typical dApp: a dashboard that silently
renders a "supported" control for a capability the deployment doesn't
actually have can walk an investor into building a transaction that reverts,
or worse, mask a compliance-relevant feature gap.

`check_interface_compatibility` answers the question directly, in one call,
from the deployment itself — the same design principle as the capability
flags it depends on.

## API

Pure read: **no storage writes, no events, no authorization required, and it
never panics** — including before `initialize` has been called and while the
contract is paused. Safe to call from a read-only RPC simulation at any time.

### `check_interface_compatibility(client_schema_version: u32, required_capabilities: Vec<Symbol>) -> InterfaceCompatibilityReport`

```rust
pub struct InterfaceCompatibilityReport {
    pub contract_schema_version: u32,       // this deployment's CAPABILITY_SCHEMA_VERSION
    pub client_schema_version: u32,         // echoed back from the call
    pub schema_relation: SchemaVersionRelation,
    pub unsupported_required: Vec<Symbol>,  // subset of the input not Supported
    pub compatible: bool,                   // true iff unsupported_required is empty
}

pub enum SchemaVersionRelation {
    Matching,     // client_schema_version == contract_schema_version
    ClientOlder,  // client_schema_version <  contract_schema_version
    ClientNewer,  // client_schema_version >  contract_schema_version
}
```

* `client_schema_version` — the [`CAPABILITY_SCHEMA_VERSION`](capabilities.md#versioning)
  the calling SDK/dashboard build was written against. Pass the constant your
  generated client was built with.
* `required_capabilities` — the capability keys (see the
  [key registry](capabilities.md#supports_capabilitycapability-symbol---capabilitystatus))
  your client build cannot function without. Pass only what is actually
  required for the feature set you are about to enable — not every key in the
  registry.
* `unsupported_required` is derived by calling `supports_capability` for each
  requested key, so it can never disagree with `get_capabilities` /
  `supports_capability`. A key resolves into this list if it is
  `Planned`, `Unsupported`, **or unknown to this deployment** — an unknown key
  fails safe exactly like `supports_capability` does.
* `compatible` is `true` **iff `unsupported_required` is empty.** A schema
  version mismatch alone never makes a client incompatible: schema fields and
  keys are append-only (see [Versioning](capabilities.md#versioning)), so the
  only thing that can actually break a client is a *specific capability it
  depends on* not being `Supported`.

## Reading `schema_relation`

| Relation | Meaning | What to do |
| --- | --- | --- |
| `Matching` | Client and deployment were built against the same schema. | Nothing extra — the two evolved together. |
| `ClientOlder` | The deployment may advertise fields/keys the client predates. | Safe on its own. Fields are append-only, so nothing the client already understands has moved or been repurposed. |
| `ClientNewer` | The client may expect fields/keys this deployment predates. | Not automatically fatal — check `unsupported_required`. If it's empty, everything the client actually asked for is present; the client simply also knows about capabilities this deployment hasn't shipped yet. |

`schema_relation` is a diagnostic signal, not a pass/fail gate by itself —
`compatible` is the field to branch on.

## SDK and dashboard usage

* **Call once per deployment, before first use.** Build `required_capabilities`
  from the feature set your build actually depends on (e.g. `whitelist`,
  `transfers`, `holding_cap`), not the full registry.
* **Branch only on `compatible`.** If `false`, block the affected flows and
  surface `unsupported_required` to the integrator/operator — it is the exact
  list to act on, not a hint to go re-derive.
* **Treat `ClientNewer` with an otherwise-empty `unsupported_required` as
  fine.** It only means the client's build knows about capabilities this
  particular deployment hasn't shipped — none of which the client currently
  requires.
* **Re-check after a contract upgrade**, the same way you would re-read
  `get_capabilities` — static capabilities are fixed per build, so cache
  results for the lifetime of a deployment, not across upgrades.

## Compatibility

* **Purely additive.** No existing function, error code, event, or storage
  key changed. The check re-derives every answer from the existing
  `supports_capability` helper, so it cannot disagree with `get_capabilities`
  or the key registry.
* **No new storage keys and no new error codes.** The function is a pure
  computation over its inputs and existing capability state.
* **Not a state-changing call**, exempt from the pause guard by design,
  consistent with the other read helpers in
  [`contract-spec.md`](contract-spec.md#read-functions).
* Tests covering matching/older/newer schema relations, aggregation of
  multiple unsupported keys, agreement with `supports_capability`, the
  empty-requirements case, and the no-mutation/pre-`initialize` guarantee
  live in [`src/test.rs`](../src/test.rs) under
  "Public interface compatibility checks (#37)".
