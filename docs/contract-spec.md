# Contract API Specification

* `initialize(env, admin)`: Sets the initial admin. Reverts if already initialized.
* `whitelist_user(env, admin, user)`: Adds `user` to the persistent compliance map. Requires admin auth. Clears prior revocation flag, allowing re-onboarding.
* `revoke_user(env, admin, user)`: Revokes/suspends a previously whitelisted investor. Requires admin auth. Sets whitelist to false and revoked to true. Emits `wl_rev` event.
* `unrevoke_user(env, admin, user)`: Clears the revoked flag without automatically re-whitelisting. Admin must call `whitelist_user` afterwards to restore privileges. Utility for two-step compliance restoration.
* `is_whitelisted_check(env, user) -> bool`: View helper - returns true if user is whitelisted and not revoked.
* `is_revoked_check(env, user) -> bool`: View helper - returns true if user is revoked.
* `compliance_status(env, user) -> (bool, bool)`: Returns `(is_whitelisted, is_revoked)` tuple for off-chain querying.
* `mint_asset(env, admin, to, amount)`: Mints `amount` to `to`. Requires admin auth. Reverts if `to` is not whitelisted or is revoked.
* `transfer(env, from, to, amount)`: Moves `amount` between addresses. Requires `from` auth. Reverts if either `from` or `to` is not whitelisted, if either is revoked, or if `from` has insufficient balance. **Policy: revoked users are fully frozen** - cannot send nor receive, but retain historical balance.
* `distribute_yield(env, admin, amount)`: Triggers a dividend yield event for off-chain indexing. Requires admin auth.

## Compliance Revocation Lifecycle

### State Model

Two persistent flags per address:

- `Whitelist(Address) -> bool` in persistent storage
- `Revoked(Address) -> bool` in persistent storage

`is_whitelisted` semantic: `Whitelist == true && Revoked == false`. `is_revoked` overrides whitelist.

### Lifecycle

1. **Never whitelisted**: both flags false/default. Cannot receive or send. Mint/transfer revert with "not whitelisted".
2. **Whitelisted**: `Whitelist=true`, `Revoked=false`. Fully operational: can receive mints, receive transfers, send transfers.
3. **Revoked**: Admin calls `revoke_user`. Sets `Whitelist=false`, `Revoked=true`. User becomes frozen:
   - Cannot receive new restricted tokens (mint to revoked reverts "Receiver is revoked")
   - Cannot receive via transfer (transfer to revoked reverts "Receiver is revoked")
   - Cannot send via transfer (transfer from revoked reverts "Sender is revoked") - **fully blocked / frozen policy**
   - Existing balance is retained (not burned) for audit/forced redemption. Holding is allowed, but no movement.
4. **Re-onboarded**: Admin calls `whitelist_user` again. This clears `Revoked=false` and sets `Whitelist=true`. User regains full privileges and can transact with retained balance. Alternatively, `unrevoke_user` clears revoked flag alone, still requiring a separate `whitelist_user` call to actually allow transactions - useful for two-step governance.

### Why fully blocked?

Alternative policy could allow revoked users to transfer out (exit only) but not in. Aegis chooses fully frozen for stronger compliance: a sanctioned or non-compliant address must not be able to offload tokens to potentially non-controlled venues without explicit admin oversight. Forced redemption or off-boarding should happen via off-chain legal process or via re-whitelisting to a compliant escrow. The policy is documented clearly and enforced at ledger level.

If a jurisdiction requires exit-only, the contract can be adapted by removing the `Sender is revoked` check in `transfer`, leaving only receiver guards. That change would be a single line and is noted here for future governance.

### Admin update path

- Only current admin (`DataKey::Admin` in instance storage) can call `whitelist_user`, `revoke_user`, `unrevoke_user`. All require `admin.require_auth()`.
- Revocation is immediate and emits `wl_rev` event.

## Emitted Events

Every state mutation publishes a contract event so off-chain systems can index
protocol activity in real time. All events are namespaced with `aegis` as
topic 0, so a single Soroban RPC topic filter captures the whole protocol,
while topic 1 (the action) narrows to one event type. Addresses are indexed as
topics so the RPC can filter by counterparty. Topic counts stay within the
Soroban limit of four.

| Event | Topics | Data | Description |
| --- | --- | --- | --- |
| `Init` | `("aegis", "init")` | `admin: Address` | Contract deployed |
| `WhitelistAdd` | `("aegis", "wl_add", user)` | `admin: Address` | User whitelisted / re-whitelisted |
| `WhitelistRevoked` | `("aegis", "wl_rev", user)` | `admin: Address` | User revoked/suspended - compliance critical |
| `Mint` | `("aegis", "mint", to)` | `[amount, new_balance, total_supply]` | Tokens minted to whitelisted |
| `Transfer` | `("aegis", "transfer", from, to)` | `amount: i128` | Transfer between whitelisted |
| `YieldDistributed` | `("aegis", "yield")` | `[admin, amount, total_supply]` | Yield distribution event |

Events are declared with `#[contractevent]` in `src/events.rs`, which generates
the topic/data encoding and includes the schema in the contract spec.

The off-chain consumer for these events lives in `monitoring/`.

### Revocation event handling off-chain

When `wl_rev` is observed, monitoring should:
- Mark user as non-compliant and frozen in compliance DB
- Alert risk/compliance desk
- Prevent UI from allowing new investment
- Trigger potential forced redemption workflow
- Log for audit trail

Re-whitelisting emits `wl_add` again, which monitoring can use to clear the frozen flag.
