# Protocol Architecture

## Separation of Concerns
The Aegis smart contract logic is cleanly modularized to separate state constraints from business logic:
* **`compliance.rs`:** Handles all Access Control Lists (ACL). Admin keys and Whitelist registries are managed here.
* **`asset.rs`:** Handles mathematical balances and total supply management. It strictly queries the compliance module before executing state changes.

## Ledger State Storage
Soroban utilizes three storage types. Aegis manages state as follows:
* **Instance Storage:** `Admin` address and `TotalSupply`. These are bound to the lifecycle of the contract instance.
* **Persistent Storage:** `Whitelist` status and User `Balance`. These must persist independently and be rent-exempted appropriately to ensure user balances are never archived unexpectedly.