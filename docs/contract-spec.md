# Contract API Specification

* `initialize(env, admin)`: Sets the initial admin. Reverts if already initialized.
* `whitelist_user(env, admin, user)`: Adds `user` to the persistent compliance map. Requires admin auth.
* `mint_asset(env, admin, to, amount)`: Mints `amount` to `to`. Requires admin auth. Reverts if `to` is not whitelisted.
* `transfer(env, from, to, amount)`: Moves `amount` between addresses. Requires `from` auth. Reverts if either `from` or `to` is not whitelisted, or if `from` has an insufficient balance.
* `distribute_yield(env, admin, amount)`: Triggers a dividend yield event for off-chain indexing. Requires admin auth.

## Emitted Events

Every state mutation publishes a contract event so off-chain systems can index
protocol activity in real time. All events are namespaced with `aegis` as
topic 0, so a single Soroban RPC topic filter captures the whole protocol,
while topic 1 (the action) narrows to one event type. Addresses are indexed as
topics so the RPC can filter by counterparty. Topic counts stay within the
Soroban limit of four.

| Event | Topics | Data |
| --- | --- | --- |
| `Init` | `("aegis", "init")` | `admin: Address` |
| `WhitelistAdd` | `("aegis", "wl_add", user)` | `admin: Address` |
| `Mint` | `("aegis", "mint", to)` | `[amount, new_balance, total_supply]` |
| `Transfer` | `("aegis", "transfer", from, to)` | `amount: i128` |
| `YieldDistributed` | `("aegis", "yield")` | `[admin, amount, total_supply]` |

Events are declared with `#[contractevent]` in `src/events.rs`, which generates
the topic/data encoding and includes the schema in the contract spec.

The off-chain consumer for these events lives in `monitoring/`.

