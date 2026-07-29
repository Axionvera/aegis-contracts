# Contract API Specification

The canonical, normative contract reference is the
[Public API and Compatibility Policy](public-api.md). It documents:

- every public function, ordered input, output, authorization rule, failure, and
  state effect;
- the exact event topic/data wire formats;
- storage and upgrade implications;
- stable, experimental, and internal surfaces;
- SDK/dashboard expectations; and
- breaking-change, versioning, deprecation, and review requirements.

## Quick index

### Stable functions

- [`initialize(admin: Address) -> ()`](public-api.md#initialize)
- [`whitelist_user(admin: Address, user: Address) -> ()`](public-api.md#whitelist_user)
- [`mint_asset(admin: Address, to: Address, amount: i128) -> ()`](public-api.md#mint_asset)
- [`transfer(from: Address, to: Address, amount: i128) -> ()`](public-api.md#transfer)

### Experimental functions

- [`distribute_yield(admin: Address, amount: i128) -> ()`](public-api.md#distribute_yield)

### Events

- Stable: [`Init`](public-api.md#init),
  [`WhitelistAdd`](public-api.md#whitelistadd),
  [`Mint`](public-api.md#mint), and
  [`Transfer`](public-api.md#transfer-1)
- Experimental: [`YieldDistributed`](public-api.md#yielddistributed)

The embedded contract spec in a released WASM is the machine-readable source
for generated bindings. Do not infer the external ABI from Rust-only helpers or
raw storage keys.
