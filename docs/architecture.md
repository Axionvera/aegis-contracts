# Protocol Architecture

## Separation of Concerns
The Aegis smart contract logic is cleanly modularized to separate state constraints from business logic:
* **`compliance.rs`:** Handles all Access Control Lists (ACL). Admin keys and Whitelist registries are managed here.
* **`asset.rs`:** Handles mathematical balances and total supply management. It strictly queries the compliance module before executing state changes.

## Ledger State Storage
Soroban utilizes three storage types. Aegis manages state as follows:
* **Instance Storage:** `Admin` address and `TotalSupply`. These are bound to the lifecycle of the contract instance.
* **Persistent Storage:** `Whitelist` status and User `Balance`. These must persist independently and be rent-exempted appropriately to ensure user balances are never archived unexpectedly.

## Event Layer
`events.rs` defines the protocol's canonical event surface. Contract modules
never call `env.events()` directly; they delegate to helpers in `events.rs` so
the emitted topic layout has exactly one source of truth. This matters because
the topic shape is a public API: off-chain filters, alert rules and analytics
all key off it, and an accidental change would silently break indexing.

## Off-Chain Monitoring Tier
`monitoring/` is a Node service that turns the on-chain event stream into
operational signal:

* **Streaming** — `SorobanEventStream` consumes Soroban RPC over a WebSocket
  subscription where one is available, and transparently degrades to
  cursor-driven `getEvents` polling otherwise. Public Soroban RPC does not yet
  ship a subscription API, so the polling path is what keeps streaming working
  against stock infrastructure today.
* **Normalization** — ScVal XDR is decoded into a stable envelope with exact
  BigInt `i128` amounts and strkey-encoded addresses.
* **Processing** — persistence (JSONL + replay + cursor checkpoints), rolling
  analytics, filtering/routing, pattern-based alerting, and event-driven
  triggers.
* **Presentation** — an HTTP API plus a WebSocket fan-out feeding a live
  dashboard.

The tier is strictly read-only with respect to the ledger: it observes events
and never submits transactions, so it cannot affect contract state.

