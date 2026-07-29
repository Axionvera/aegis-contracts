# Aegis Contract Public API and Compatibility Policy

**Applies to:** `aegis-contracts` `0.1.x`  
**Policy status:** normative from the first release that contains this document

This document is the stable reference for contract integrators. It defines the
on-chain function ABI, successful outputs, failure conditions, event wire
format, storage implications, and the compatibility process for contract, SDK,
dashboard, and indexer changes.

The contract spec embedded in a released WASM is the machine-readable authority
for names and Soroban types. This policy is the authority for behavioral and
compatibility promises. If a released WASM and this document disagree, treat
that as a release defect: integrations must follow the deployed WASM until the
release is corrected.

## Stability classification

Every externally relevant surface has one of these statuses:

| Status | Meaning | Compatibility promise |
| --- | --- | --- |
| **Stable** | Supported for production integrations. | No incompatible change inside the `0.1.x` release line. A breaking change follows the versioning, deprecation, and review process below. |
| **Experimental** | Available for evaluation, but its semantics or shape may change. | May change in a minor release after release-note and downstream-owner review. It must not change incompatibly in a patch release. Consumers should feature-detect or pin a contract release. |
| **Internal** | An implementation detail, including Rust helpers and raw storage layout. | No direct consumer compatibility promise. Upgrades must still preserve existing ledger state or provide a reviewed migration. |

The status applies to the complete wire contract: function/event name, argument
or field order, Soroban types, output, authorization requirements, documented
failure conditions, and documented semantics. A Rust item being `pub` does not
make it an on-chain public API; only methods exported through
`#[contractimpl]` and contract events are externally supported.

### Current surface

| Surface | Status |
| --- | --- |
| `initialize`, `whitelist_user`, `mint_asset`, `transfer` | **Stable** |
| `Init`, `WhitelistAdd`, `Mint`, `Transfer` event wire formats | **Stable** |
| `distribute_yield` and `YieldDistributed` | **Experimental** — the function only emits an indexing signal; it does not distribute or escrow assets |
| `DataKey`, Rust modules/helpers, concrete storage keys, panic text, and private implementation details | **Internal** |

## Calling conventions

- The `Env` parameter visible in Rust is supplied by Soroban and is **not** an
  SDK argument.
- `Address` inputs are Soroban addresses. Required authorization is stated per
  function and is enforced by `Address::require_auth`.
- `i128` values are signed 128-bit integers. JavaScript/TypeScript consumers
  must use exact `bigint`/SDK integer representations, never `number`.
- Every current entry point returns Soroban unit/void (`()`). A successful SDK
  invocation therefore has no application return value.
- State writes and events are atomic. A failed invocation commits neither.
- Events listed below are contract events. Consumers should process only events
  from successful, finalized transactions and from the expected contract ID.

## Public function reference

### `initialize`

**Status:** Stable  
**SDK shape:** `initialize(admin: Address) -> ()`

| Input | Type | Rules |
| --- | --- | --- |
| `admin` | `Address` | Becomes the sole administrative address for later privileged calls. |

**Authorization:** None. The current implementation does not call
`admin.require_auth()`. Deployment tooling must initialize a new contract
atomically or immediately and must not expose an uninitialized instance to an
untrusted caller.

**Success effects:**

1. Writes `admin` to instance storage under `DataKey::Admin`.
2. Emits one `Init` event.
3. Returns `()`.

**Failure:** Fails with diagnostic `Contract already initialized` if the admin
key already exists. Initialization can succeed only once for a given state.

### `whitelist_user`

**Status:** Stable  
**SDK shape:** `whitelist_user(admin: Address, user: Address) -> ()`

| Input | Type | Rules |
| --- | --- | --- |
| `admin` | `Address` | Must authorize the invocation and equal the initialized admin. |
| `user` | `Address` | Address to mark as whitelisted. Any Soroban address is accepted. |

**Authorization:** `admin` authorization is required.

**Success effects:**

1. Writes `true` to persistent storage under `DataKey::Whitelist(user)`.
2. Emits one `WhitelistAdd` event, including when the user was already
   whitelisted (the write is idempotent, the event is not suppressed).
3. Returns `()`.

**Failure:**

- Host authorization failure if valid `admin` authorization is absent.
- Uninitialized-state trap if `DataKey::Admin` does not exist.
- `Unauthorized: Only admin can whitelist` if `admin` is not the stored admin.

There is currently no public removal or whitelist-query function.

### `mint_asset`

**Status:** Stable  
**SDK shape:** `mint_asset(admin: Address, to: Address, amount: i128) -> ()`

| Input | Type | Rules |
| --- | --- | --- |
| `admin` | `Address` | Must authorize the invocation and equal the initialized admin. |
| `to` | `Address` | Must already be whitelisted. |
| `amount` | `i128` | Must be strictly greater than zero. |

**Authorization:** `admin` authorization is required.

**Success effects:**

1. Adds `amount` to persistent `DataKey::Balance(to)` (a missing balance is
   treated as zero).
2. Adds `amount` to instance `DataKey::TotalSupply` (a missing supply is treated
   as zero).
3. Emits one `Mint` event with the delta, resulting balance, and resulting total
   supply.
4. Returns `()`.

**Failure:**

- Host authorization failure if valid `admin` authorization is absent.
- Uninitialized-state trap if `DataKey::Admin` does not exist.
- `Unauthorized: Only admin can mint` if `admin` is not the stored admin.
- `Amount must be positive` if `amount <= 0`.
- `Receiver is not whitelisted` if `to` is not whitelisted.
- Arithmetic/host trap if an `i128` addition overflows.

### `transfer`

**Status:** Stable  
**SDK shape:** `transfer(from: Address, to: Address, amount: i128) -> ()`

| Input | Type | Rules |
| --- | --- | --- |
| `from` | `Address` | Must authorize the invocation, be whitelisted, and have at least `amount`. |
| `to` | `Address` | Must be whitelisted. |
| `amount` | `i128` | Must be strictly greater than zero. |

**Authorization:** `from` authorization is required.

**Success effects:**

1. Subtracts `amount` from persistent `DataKey::Balance(from)`.
2. Adds `amount` to persistent `DataKey::Balance(to)`.
3. Leaves total supply unchanged.
4. Emits one `Transfer` event.
5. Returns `()`.

If `from == to`, the final balance is unchanged, but authorization, compliance,
amount, and balance checks still run and a `Transfer` event is emitted.

**Failure:**

- Host authorization failure if valid `from` authorization is absent.
- `Amount must be positive` if `amount <= 0`.
- `Sender is not whitelisted` if `from` is not whitelisted.
- `Receiver is not whitelisted` if `to` is not whitelisted.
- `Insufficient balance` if the stored/missing-as-zero sender balance is less
  than `amount`.
- Arithmetic/host trap if an `i128` operation overflows.

### `distribute_yield`

**Status:** Experimental  
**SDK shape:** `distribute_yield(admin: Address, amount: i128) -> ()`

| Input | Type | Rules |
| --- | --- | --- |
| `admin` | `Address` | Must authorize the invocation and equal the initialized admin. |
| `amount` | `i128` | Must be strictly greater than zero. It is an event value, not an asset transfer. |

**Authorization:** `admin` authorization is required.

**Success effects:** Reads total supply (missing-as-zero), emits one
`YieldDistributed` event, and returns `()`. It does **not** change balances,
total supply, or a claimable-yield ledger entry. SDKs and dashboards must not
represent this call as proof that funds were paid.

**Failure:**

- Host authorization failure if valid `admin` authorization is absent.
- Uninitialized-state trap if `DataKey::Admin` does not exist.
- `Unauthorized` if `admin` is not the stored admin.
- `Amount must be positive` if `amount <= 0`.

## Error model and failure reference

The current contract defines no `#[contracterror]` enum and returns no typed
`Result`. Its explicit validation failures are assertion traps. Soroban host
failures (including missing authorization, budget exhaustion, archived storage,
and malformed input) use host-defined error values.

| Function | Failure condition | Current diagnostic/category |
| --- | --- | --- |
| `initialize` | Already initialized | `Contract already initialized` |
| `whitelist_user` | Caller-supplied admin differs | `Unauthorized: Only admin can whitelist` |
| `mint_asset` | Caller-supplied admin differs | `Unauthorized: Only admin can mint` |
| `distribute_yield` | Caller-supplied admin differs | `Unauthorized` |
| `mint_asset`, `transfer`, `distribute_yield` | Non-positive amount | `Amount must be positive` |
| `mint_asset`, `transfer` | Receiver not whitelisted | `Receiver is not whitelisted` |
| `transfer` | Sender not whitelisted | `Sender is not whitelisted` |
| `transfer` | Sender funds too low | `Insufficient balance` |
| Privileged/sender calls | Required signature/auth tree absent | Soroban host authorization error |
| Calls requiring initialized admin | Admin key absent | Contract/host trap; no stable diagnostic |

For **stable** functions, the listed failure conditions are compatibility
promises. The exact panic strings and host error encoding are internal,
diagnostic-only details: SDKs must not parse them for application logic. SDKs
should first distinguish success from failure using the Soroban transaction
result, preserve the raw diagnostic for operators, and map failures to their
own versioned error model only when the underlying value is reliably typed.
Introducing typed contract error codes, changing a stable failure condition, or
changing an entry point to return `Result` requires explicit compatibility
review; a return-type change is breaking.

## Event reference

All events begin with `aegis` in topic position 0 and an action symbol in topic
position 1. Field order and type are part of the wire API. Bracketed data below
is a Soroban vector/tuple in exactly the displayed positional order, not a
named map.

### `Init`

**Status:** Stable  
**Emitted by:** successful `initialize`

- Topics: `("aegis", "init")`
- Data: `admin: Address`

### `WhitelistAdd`

**Status:** Stable  
**Emitted by:** successful `whitelist_user`

- Topics: `("aegis", "wl_add", user: Address)`
- Data: `admin: Address`

### `Mint`

**Status:** Stable  
**Emitted by:** successful `mint_asset`

- Topics: `("aegis", "mint", to: Address)`
- Data: `[amount: i128, new_balance: i128, total_supply: i128]`

### `Transfer`

**Status:** Stable  
**Emitted by:** successful `transfer`

- Topics: `("aegis", "transfer", from: Address, to: Address)`
- Data: `amount: i128`

### `YieldDistributed`

**Status:** Experimental  
**Emitted by:** successful `distribute_yield`

- Topics: `("aegis", "yield")`
- Data: `[admin: Address, amount: i128, total_supply: i128]`

The Rust event struct names aid generated bindings, while topics and encoded
data are the indexer's wire contract. Stable consumers may depend on one event
per successful stable state-changing call and on the topic/data layouts above.
They must ignore unknown action topics so new functions can introduce new event
types additively.

## Storage implications

There is no supported direct-storage API. SDKs, dashboards, and third-party
contracts must not construct `DataKey` values or depend on enum discriminants,
ledger-key XDR, or storage internals. The current layout is documented here so
contract maintainers can review upgrades and migrations—not as a query
interface for consumers.

| Key | Soroban storage | Value | Writers/readers | Consumer-visible implication |
| --- | --- | --- | --- | --- |
| `DataKey::Admin` | Instance | `Address` | Written once by `initialize`; read by privileged calls | Determines who may whitelist, mint, and signal yield. No public getter exists. |
| `DataKey::Whitelist(Address)` | Persistent | `bool` | Written by `whitelist_user`; read by mint/transfer checks | Missing means `false`. No public getter/removal exists. |
| `DataKey::Balance(Address)` | Persistent | `i128` | Written by mint/transfer | Missing means zero. No public balance getter exists. |
| `DataKey::TotalSupply` | Instance | `i128` | Written by mint; read by mint/yield | Missing means zero. No public supply getter exists. |

Current code does not explicitly extend storage TTL or expose restoration
methods. Persistent and instance entries are therefore subject to Stellar
ledger TTL/archive behavior. Operators must manage deployment/state lifecycle;
consumers must not interpret “persistent” as “permanent.”

Changing a storage key's encoding, `DataKey` variant name/payload, value type,
or storage class is internal from a caller-ABI perspective but
migration-sensitive.
An upgrade that reuses existing state must preserve readable values or ship and
test an atomic migration. Resetting balances, supply, whitelist, or admin is a
breaking state-compatibility change even when function signatures are unchanged.

### State/write matrix

| Function | Admin | Whitelist | Balances | Total supply |
| --- | --- | --- | --- | --- |
| `initialize` | Write once | — | — | — |
| `whitelist_user` | Read | Write `user = true` | — | — |
| `mint_asset` | Read | Read `to` | Add to `to` | Add amount |
| `transfer` | — | Read `from`, `to` | Subtract/add | Unchanged |
| `distribute_yield` | Read | — | Unchanged | Read only |

## SDK expectations

SDK maintainers must:

1. Generate bindings from the exact released WASM/spec and identify the release
   and deployed contract ID they target.
2. Preserve argument order and use exact `i128` representations. In JavaScript
   and TypeScript, expose `bigint` or decimal strings at JSON boundaries.
3. Build the required auth entries: `admin` for privileged methods and `from`
   for `transfer`. Do not assume `initialize` currently authenticates `admin`.
4. Treat `()` as the only successful output and use finalized transaction status
   to determine success.
5. Avoid branching on panic strings or reading raw contract storage.
6. Mark `distribute_yield` experimental and describe it as event-only.
7. Pin a compatible contract release; regenerate and test bindings when the
   embedded spec changes.

The Rust-generated `AegisContractClient` is a development/test client. Its Rust
module paths and helper method implementation are not the cross-language API;
the embedded contract spec and this document are.

## Dashboard and indexer expectations

Dashboard/indexer maintainers must:

1. Filter by the deployed contract ID and `topics[0] == "aegis"`, dispatch on
   topic 1, validate topic count/types, and safely ignore unknown actions.
2. Decode `i128` without precision loss and retain event order/cursor metadata
   for replay and idempotency.
3. Advance derived state only from successful, finalized ledger events. Handle
   duplicate delivery and ledger reprocessing idempotently.
4. Use `Mint.total_supply`/`new_balance` only as the post-call values for that
   event. A `Transfer` does not change supply.
5. Treat `WhitelistAdd` as an add signal only; there is currently no removal
   event. Repeated adds are valid events.
6. Label `YieldDistributed` as an experimental notification, not settlement or
   proof of payment.
7. Reindex or migrate stored projections when adopting an incompatible contract
   release. Event-derived views are projections, not authoritative storage
   queries.

Because the contract currently exposes no read methods, SDK/dashboard teams
must not invent unsupported balance, supply, admin, or whitelist query
contracts. A future read API must be proposed and versioned as a new public
surface.

## Breaking-change categories

A change is **breaking** when an existing conforming consumer of a stable
surface must change code, regenerate incompatible bindings, reinterpret stored
events, or migrate existing contract state. Examples include:

### Function ABI

- Renaming/removing an entry point.
- Reordering, adding, removing, renaming, or changing the type of an argument.
- Changing `()` to another return type or changing the encoded output.
- Adding an authorization requirement (including authenticating
  `initialize`), changing the authorized address, or materially tightening a
  documented precondition.
- Changing token/accounting semantics, such as applying a transfer fee or
  making mint affect a different balance/supply model.

### Errors

- Removing or changing a documented stable failure condition in a way that
  changes caller behavior.
- Replacing traps with typed errors, assigning stable error codes, or changing
  the output to `Result` without a migration/version plan for generated SDKs.
- Panic wording alone is not breaking because panic strings are internal.

### Events

- Changing namespace/action symbols, topic count/order/type, indexed addresses,
  data encoding, data field order/type, or the meaning of an existing field.
- Removing a stable event or ceasing to emit it for the documented successful
  call.
- Emitting additional events from an existing stable call when this violates
  the documented one-event cardinality.
- Adding an event for a new function is additive if consumers can ignore unknown
  actions.

### State and upgrades

- Making existing state unreadable without migration; moving keys between
  storage classes without migration; resetting admin, whitelist, balances, or
  supply; or changing units/decimals/accounting interpretation.
- Deploying a replacement at a new contract ID without giving consumers a
  release/deployment migration path.

The following are normally **non-breaking**: documentation corrections that do
not change behavior, internal refactors with identical WASM behavior, bug or
security fixes that restore documented behavior, and new functions/events that
do not alter existing calls and can be ignored by older consumers. Adding a
new stable surface requires a minor version; compatible fixes use a patch.
Ambiguous semantic changes must be treated as breaking until the compatibility
review determines otherwise.

Experimental surfaces may change without preserving their old shape in the
next minor release, but the change must be called out and coordinated. Internal
changes need no consumer compatibility guarantee, except for the state
migration requirements above.

## Versioning and deprecation

Aegis uses Semantic Versioning for the crate, released WASM, contract spec, and
this public API as one unit:

- **Current pre-1.0 line:** patch releases (`0.1.z`) preserve all stable
  surfaces. A stable breaking change increments the minor version (`0.2.0`).
  A new stable surface also increments the minor version. Experimental breaking
  changes are allowed only in a minor release.
- **At/after 1.0:** patch = compatible fix, minor = compatible addition, major =
  stable breaking change.
- Each release should be tagged `vMAJOR.MINOR.PATCH` and publish the WASM hash,
  embedded contract spec, network contract IDs (when deployed), API changes,
  migrations, and downstream actions in release notes.
- A deployed contract is identified by both its contract ID/network and its
  release/WASM hash. The Cargo version alone does not prove what is deployed.

Before removing or incompatibly changing a stable surface, maintainers should
mark it deprecated in this document and release notes for at least one published
minor release where feasible, provide the replacement and migration guidance,
and notify SDK/dashboard owners. A critical security or ledger-safety fix may
shorten the window, but must still receive the breaking-change reviews and a
clear security advisory/release note.

There is currently no on-chain `api_version()` entry point. Consumers should
use release/deployment metadata rather than probing an undocumented value.

## Change review requirements

Every PR that touches `#[contractimpl]`, `#[contractevent]`, authorization,
validation/failure behavior, `DataKey`, storage operations, or accounting must
complete this review before merge:

1. **Classify** each changed surface as stable, experimental, or internal and
   label the PR as compatible, additive, experimental-breaking, or
   stable-breaking.
2. **Diff the ABI/spec** generated by the candidate WASM against the latest
   release: function names, ordered inputs/types, outputs, event topics/data,
   and errors.
3. **Update this document** and any generated/reference contract spec in the
   same PR. Update release notes/version when required.
4. **Test behavior** with `cargo fmt --all -- --check`, `cargo clippy
   --all-targets`, `cargo test`, and `make build`. Add focused tests for auth,
   failures, state transitions, and exact event XDR/topic/data shape.
5. **Review state compatibility.** For a storage change, include migration code,
   rollback/forward plan, fixture state from the previous release, and tests
   proving admin/whitelist/balances/supply survive.
6. **Review downstream impact.** Stable-breaking and event changes require
   explicit approval from a contract maintainer and designated SDK and
   dashboard/indexer owners. Include regenerated binding and decoder/projection
   test results or links to coordinated downstream PRs.
7. **Plan rollout.** Record deployment IDs/WASM hash, order of contract and
   consumer releases, reindex requirements, monitoring, and rollback limits.

A PR is not “internal only” merely because its Rust signature is unchanged.
Authorization, event encoding, failure behavior, accounting meaning, and state
migration are all compatibility surfaces and must be reviewed.

## Maintainer checklist

Copy this checklist into API-affecting PRs:

```text
[ ] Stability and breaking-change classification recorded
[ ] Released WASM/spec diff reviewed
[ ] Public API policy/reference updated
[ ] Function input/output/auth/failure tests updated
[ ] Event wire-format fixtures/tests updated
[ ] Existing-state migration tested or marked not applicable
[ ] SDK owner approval obtained or marked not applicable
[ ] Dashboard/indexer owner approval obtained or marked not applicable
[ ] Version, release notes, deployment, reindex, and rollback plan updated
[ ] fmt, clippy, cargo test, and WASM build pass
```
