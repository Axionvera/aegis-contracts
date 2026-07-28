# SDK Integration Fixtures

Deterministic, machine-readable examples of what the Aegis RWA contracts
actually return — compliance reads, minting, transfers, events, errors, and
capability reads — published for use by other repositories.

The fixtures live in [`fixtures/sdk/`](../fixtures/sdk) and are generated and
verified by [`tests/sdk_fixtures.rs`](../tests/sdk_fixtures.rs).

## Why these exist

SDK, dashboard, and indexer repos all need to know the exact shape of a
contract response before they can write a test: the topic string on a mint
event, the field name inside its payload, the numeric code behind a rejected
transfer. Without a shared source of truth, each repo hand-writes its own
mock. Those mocks drift from the contract, and the drift is usually only
discovered against a live network — the slowest and most expensive place to
find it.

These fixtures give every downstream repo one canonical, versioned answer.
They complement the two existing interface documents rather than replacing
them:

| Document | Answers |
|---|---|
| [`docs/events.md`](events.md) | *What* events exist and what their fields mean |
| [`docs/error-codes.md`](error-codes.md) | *What* each numeric error code means |
| **`docs/sdk-fixtures.md`** (this file) | *What the bytes actually look like* |

## How they are produced

Every value is captured from a **real contract invocation** running in the
Soroban test host, then serialised from the wire-level `ScVal`/XDR that an SDK
would receive. Nothing in `fixtures/sdk/` is hand-written, so a fixture cannot
describe behaviour the contract does not have.

Determinism comes from three choices:

1. The contract is registered at a **fixed address** rather than a
   host-generated one.
2. Every actor address is a **fixed synthetic strkey** (see below).
3. Fixtures are rendered with a small, insertion-ordered JSON writer, so byte
   output is stable across runs, machines, and Rust versions.

## The two modes

The harness both publishes and guards the fixtures.

```bash
# Verify (the default, and what CI runs):
# regenerate every scenario and compare byte-for-byte against the
# committed files. Contract drift fails here.
cargo test --test sdk_fixtures

# Update: rewrite the committed fixtures after an intentional change.
UPDATE_FIXTURES=1 cargo test --test sdk_fixtures
```

Because verification is the default, a change in observable contract
behaviour — a renamed event topic, a reordered check, a changed error code —
fails the test suite in *this* repo, instead of silently shipping a stale
fixture to the SDK. When a failure is intentional, regenerate, **review the
diff**, and commit it as part of the same change.

## Files

| File | Covers |
|---|---|
| `00-actors.json` | The synthetic identity table shared by every other fixture |
| `01-compliance.json` | Whitelist reads/writes, revocation, role reads |
| `02-minting.json` | Issuance, running total supply, supply & holding cap governance |
| `03-transfers.json` | Transfers plus the pre-flight eligibility helpers |
| `04-events.json` | One canonical example of every event topic |
| `05-errors.json` | Every reachable error code, from a real failing call |
| `06-capabilities.json` | The read-only surface: balances, caps, pause, status, metadata |

Each file shares one envelope:

```jsonc
{
  "$schema_version": 1,
  "fixture": "04-events",
  "purpose": "...",
  "contract": "CCEOFPHM...",
  "generator": "tests/sdk_fixtures.rs (cargo test --test sdk_fixtures)",
  "notes": ["..."],
  "scenarios": [
    { "id": "event-asset-minted", "description": "...", "...": "..." }
  ]
}
```

Address a scenario by its `id`, which is unique within a file and stable
across regenerations. `$schema_version` is bumped only if the envelope shape
itself changes.

## No real user data

This is a hard requirement, enforced by a test rather than by convention.

Every address is **synthetic and derived from a published formula**: the
Ed25519 public key bytes are literally `SHA-256("aegis-fixture/<label>")`,
encoded as a Stellar strkey. For example:

```
sha256("aegis-fixture/investor-alice")
  = 2f1a83df1f5ab629fe3f51171768c748c85bc08052b9d11330019a5d86c7c587
  → GAXRVA67D5NLMKP6H5IROF3IY5EMQW6AQBJLTUITGAAZUXMGY7CYO2KG
```

Consequences worth stating plainly:

- **No private key exists** for any of these addresses, and none can be
  derived — they are hashes of a public string, not generated keypairs.
- They are unfunded on every network and correspond to **no real person,
  account, or KYC record**.
- Because the derivation is public, any repo can reproduce the exact same
  address set instead of inventing its own.

The `fixtures_contain_no_real_user_data` test enforces this on every run. It
asserts that every `G…`/`C…` strkey appearing in a fixture comes from the
synthetic actor table, that nothing shaped like a Stellar secret seed (`S…`)
appears anywhere — including inside XDR blobs — that no personal-data or
live-network terms appear, and that the only URL host referenced is the
IANA-reserved `example.invalid`.

Note that the contract itself never stores personal data: the on-chain
compliance model is a boolean whitelist flag per address, with all KYC
evidence held off-chain (see
[`docs/legal-boundary-disclaimer.md`](legal-boundary-disclaimer.md)). The
fixtures inherit that property.

## Value encoding

Fixtures render wire values into JSON with these rules. They matter because a
naive `JSON.parse` in a JavaScript SDK will otherwise corrupt balances.

| Contract type | JSON | Notes |
|---|---|---|
| `i128`, `u128`, `i64`, `u64` | **decimal string** — `"1000"` | Prevents silent precision loss in IEEE-754 consumers. Parse with `BigInt`. |
| `u32`, `i32` | number — `1000` | Always exactly representable. |
| `bool` | `true` / `false` | |
| `Address` | strkey string | `"GAXRVA67…"` |
| `String`, `Symbol` | string | |
| `Bytes` | lowercase hex string | |
| unit enum (`Role`, `AssetStatus`) | **single-element array** — `["Admin"]` | This is the real wire encoding of a `#[contracttype]` unit enum, not a quirk of the renderer. |
| struct | object keyed by field name | Field order follows the wire map (sorted by key). |
| `Option::None`, `Void` | `null` | e.g. `remaining_capacity` when no holding cap is set. |
| contract error | `{"type":"contract","code":4001,…}` | |

Event entries additionally carry `xdr_base64`: the complete, unmodified
`ContractEvent` XDR. If you ever doubt the JSON rendering, decode that blob —
it is the ground truth the JSON was derived from, and it lets a downstream SDK
test its own XDR decoder against a known-good input.

## Reading a fixture: worked example

From `04-events.json`:

```jsonc
{
  "id": "event-asset-minted",
  "events": [{
    "contract": "CCEOFPHM2IOUTJS53R74QWIEQXXEHLYOTZYMCS44UI735A4WCJZAQNWP",
    "type": "contract",
    "topic": "asset_minted",
    "topics": ["asset_minted"],
    "data": {
      "amount": "1000",
      "caller": "GBL2V6RSU2K6U3C73HDQOCVWBCQ7H7ZJY6RECEHIUHSZ7M6736CDPBMO",
      "to": "GAXRVA67D5NLMKP6H5IROF3IY5EMQW6AQBJLTUITGAAZUXMGY7CYO2KG",
      "total_supply": "1000"
    },
    "xdr_base64": "AAAAAAAAAAGI4rzs…"
  }]
}
```

Three things an SDK author should take from this:

- Match on `topic`, never on payload field order.
- `total_supply` is the **cumulative** supply after the mint, not the amount
  minted. The `mint-running-total-supply` scenario in `02-minting.json` exists
  specifically to make that unmistakable: two mints of 400 and 600 produce a
  second event whose `amount` is `"600"` and whose `total_supply` is `"1000"`.
- `caller` is whoever authorised the action — an `AssetManager` or the Admin —
  not necessarily the Admin.

## Errors, and the two failure kinds

`05-errors.json` contains a captured example of **every variant** of the
`Error` enum. A test asserts that completeness, so a new error variant added
to `src/errors.rs` cannot ship without a fixture example.

Successful calls render as `{"ok": true}`. Failures come in two distinct
kinds, and SDKs must handle both:

```jsonc
// 1. A contract error: carries a stable numeric code.
{"ok": false, "error": {
  "type": "contract", "code": 4001,
  "name": "ReceiverNotWhitelisted", "category": "compliance"
}}

// 2. A host trap: carries NO contract code.
{"ok": false, "error": {
  "type": "host", "code": null,
  "reason": "Mint would exceed the active supply cap"
}}
```

The second kind is easy to miss. The supply cap and per-investor holding cap
are enforced with `assert!`, which traps in the host rather than returning a
contract error, so **no numeric code is available to match on**. Clients must
fail safe on these instead of assuming every failure has a code. The two
`error-host-trap-*` scenarios capture exactly this shape.

The practical mitigation is to call the read-only helpers
(`check_transfer_eligibility`, `get_investor_eligibility`) *before* submitting,
which is what the `03-transfers.json` scenarios demonstrate.

Also worth internalising: a compliance-blocked transfer emits **no events at
all**. Soroban discards events from reverted invocations, so the error code is
the only off-chain-observable signal. The
`event-none-on-reverted-transfer` scenario pins that behaviour, and
[`docs/events.md`](events.md) explains the reasoning.

## Using these downstream

Fixtures are plain JSON with no dependency on Rust or the Soroban SDK, so any
language can consume them. Vendor the directory, add it as a git submodule, or
fetch it at a pinned commit.

```ts
import fixture from "./fixtures/sdk/04-events.json";

const scenario = fixture.scenarios.find(s => s.id === "event-asset-minted")!;
const event = scenario.events[0];

expect(decodeEvent(event.xdr_base64)).toEqual({
  topic: event.topic,
  amount: BigInt(event.data.amount),      // string → BigInt, never Number
  totalSupply: BigInt(event.data.total_supply),
});
```

Two conventions to follow when depending on them: pin to a specific commit so
a contract change cannot silently alter your test expectations, and key off
`id` rather than array position.

## Adding a scenario

1. Add it to the relevant `fixture_*` test in `tests/sdk_fixtures.rs`, driving
   the real client rather than constructing values by hand.
2. Use `Harness::render` for return values and `Harness::events` for events,
   so output is derived from wire-level XDR.
3. Assert the behaviour in Rust as well (`assert_eq!`) — the fixture records
   what happened, the assertion states what *should* happen, and having both
   means a wrong fixture cannot quietly become the new expected value.
4. Give it a unique, stable `id`; ids are the downstream addressing key and
   renaming one is a breaking change.
5. Regenerate with `UPDATE_FIXTURES=1`, review the diff, and commit the
   updated JSON alongside the test change.

Use only the actors in `00-actors.json`. Adding a new one means extending the
`ACTORS` table in `tests/support/mod.rs` with a strkey derived from the
documented `SHA-256("aegis-fixture/<label>")` formula — never a real address.
