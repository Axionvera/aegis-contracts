# Protocol Architecture

## Separation of Concerns
The Aegis smart contract logic is cleanly modularized to separate state constraints from business logic:
* **`compliance.rs`:** Handles all Access Control Lists (ACL). Admin keys, Whitelist registries, and Revocation registry are managed here. Exposes `is_whitelisted`, `is_revoked`, `whitelist_user`, `revoke_user`, `unrevoke_user`, and view helpers.
* **`asset.rs`:** Handles mathematical balances and total supply management. It strictly queries the compliance module before executing state changes, including revocation guards.

## Ledger State Storage
Soroban utilizes three storage types. Aegis manages state as follows:
* **Instance Storage:** `Admin` address and `TotalSupply`. These are bound to the lifecycle of the contract instance.
* **Persistent Storage:** `Whitelist` status, `Revoked` status, and User `Balance`. These must persist independently and be rent-exempted appropriately to ensure user balances are never archived unexpectedly. `Revoked` is kept in persistent storage so historical revocation remains queryable.

### DataKey Layout
```rust
enum DataKey {
    Admin,
    Whitelist(Address),
    Revoked(Address),
    Balance(Address),
    TotalSupply,
}
```

- `Whitelist(Address) -> bool`: true if KYC approved
- `Revoked(Address) -> bool`: true if compliance revoked/suspended
- Effective whitelist: `Whitelist == true && Revoked == false`

## Revocation Lifecycle - Design Rationale

### Why separate flag vs just setting whitelist false?

Having an explicit `Revoked` flag allows:
- Distinguishing never-whitelisted vs revoked for better error messages ("Receiver is revoked" vs "not whitelisted")
- Audit trail: compliance_status view returns both flags
- Re-onboarding workflow: whitelist_user clears revocation, but we can also keep two-step via unrevoke_user
- Off-chain monitoring can filter on `wl_rev` event for alerts

### Transfer Policy: Fully Blocked (Frozen)

Revoked users:
- **Cannot receive new restricted tokens**: `mint_asset` checks `is_revoked` before `is_whitelisted` and panics "Receiver is revoked". Same for transfer-in.
- **Cannot send**: `transfer` checks `is_revoked` for sender and panics "Sender is revoked". This implements fully frozen semantics.
- **Can hold**: Balance is retained in persistent storage, not burned. Allows snapshot for yield accounting and forced redemption.
- **Can be re-whitelisted**: `whitelist_user` clears revoked flag.

Alternative considered: allow transfer-out (exit only). That would be implemented by removing sender revoked check. The current fully-blocked policy is more conservative and satisfies regulators requiring immediate freeze of sanctioned addresses. The alternative is documented in `contract-spec.md` as a one-line change if governance decides.

### Event Layer
`events.rs` defines the protocol's canonical event surface. Contract modules
never call `env.events()` directly; they delegate to helpers in `events.rs` so
the emitted topic layout has exactly one source of truth. This matters because
the topic shape is a public API: off-chain filters, alert rules and analytics
all key off it, and an accidental change would silently break indexing.

New event for revocation:
- `WhitelistRevoked` with topics `("aegis","wl_rev",user)` data `admin`

Helpers:
- `user_whitelisted`
- `user_revoked`
- `asset_minted`
- `asset_transferred`
- `yield_distributed`

All events remain namespaced with `aegis` as topic 0.

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

For revocation, monitoring should:
- On `wl_rev`: mark address frozen, trigger compliance alert, block UI investment, log audit.
- On `wl_add` after prior `wl_rev`: clear frozen flag, log re-onboarding.
- Dashboard should display revoked count and list.
