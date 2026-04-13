# Contract API Specification

* `initialize(env, admin)`: Sets the initial admin. Reverts if already initialized.
* `whitelist_user(env, admin, user)`: Adds `user` to the persistent compliance map. Requires admin auth.
* `mint_asset(env, admin, to, amount)`: Mints `amount` to `to`. Requires admin auth. Reverts if `to` is not whitelisted.
* `transfer(env, from, to, amount)`: Moves `amount` between addresses. Requires `from` auth. Reverts if either `from` or `to` is not whitelisted, or if `from` has an insufficient balance.
* `distribute_yield(env, admin, amount)`: Triggers a dividend yield event for off-chain indexing. Requires admin auth.