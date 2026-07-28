# Supply Cap Amendment Governance

This document describes the supply cap amendment governance workflow for the
Aegis RWA Contracts (issue #32). It is part of the protocol's compliance
control surface and is intended to be safe for RWA use cases.

> **Not legal or financial advice.** This is protocol-level mechanics only.
> How a supply cap maps to a real-world asset's authorized issuance, regulatory
> limits, or legal agreements is outside the scope of the contract and must be
> determined by the asset issuer's compliance and legal functions.

## Data model

The contract stores a single **global** supply cap (the Aegis contract models
one token per deployment). Two storage keys back it:

| Key | Meaning |
| --- | --- |
| `SupplyCap` | The currently active cap. A value of `0` means **no cap enforced** (unbounded minting, still subject to the compliance whitelist). |
| `SupplyCapCandidate` | A pending proposed cap awaiting 2-step acceptance. Absent when no proposal is outstanding. |

## Governance model: 2-step amendment

Changing the cap is a sensitive, compliance-relevant action. It follows the
same safe 2-step pattern used for admin transfer:

1. **Propose** — `propose_supply_cap(new_cap)` records the candidate. The
   active cap is **not** changed yet. Only the supreme admin can propose, and
   only when the contract is not paused. Proposing the same value as the active
   cap (a no-op) is rejected; negative values are rejected.
2. **Accept** — `accept_supply_cap()` promotes the candidate to the active
   cap and clears the proposal slot. Only the supreme admin can accept.
3. **Cancel** — `cancel_supply_cap_proposal()` discards an outstanding
   proposal without applying it. Only the supreme admin can cancel.

Both steps emit events (`supply_cap_proposed`, `supply_cap_amended`) so the
amendment is auditable off-chain.

## Enforcement

`mint_asset` calls `enforce_supply_cap(amount)` **before** increasing total
supply. The check runs for every caller (including the admin and
AssetManager), because the cap is a protocol-level invariant, not a role
privilege.

- If the active cap is `0`, minting is never blocked by the cap.
- If the cap is `> 0`, the mint reverts when `total_supply + amount > cap`.

## Edge cases and failure states

| Case | Behaviour |
| --- | --- |
| No cap set (default) | `get_supply_cap()` returns `0`; minting is unbounded (whitelist still enforced). |
| Propose by non-admin | Reverts (`Unauthorized: only admin can propose a supply cap`). |
| Accept with no proposal | Reverts (`No pending supply cap proposal to accept`). |
| Cancel with no proposal | Reverts (`No pending supply cap proposal to cancel`). |
| Proposed cap equals active cap | Reverts (`Proposed cap equals the active cap — no change requested`). |
| Negative proposed cap | Reverts (`Supply cap must be non-negative`). |
| Cap lowered below current supply | Allowed. Existing supply is **not** burned; further mints that would exceed the (lower) cap are blocked until supply falls or the cap is raised. |
| Mint would exceed cap | Reverts (`Error::SupplyCapExceeded`, code `5002`). |
| Contract paused | All cap governance calls and `mint_asset` revert. |

## Why 2-step (and not immediate)?

An immediate cap change — especially one set too low, or set to `0` after a
high cap — could brick minting or, conversely, remove a safeguard. Requiring an
explicit accept step gives the admin a deliberate checkpoint and a window to
cancel a mistaken proposal, mirroring the admin-transfer safety model already
in `admin.rs`.

## Compatibility

- The workflow reuses the existing RBAC (`get_admin`, `require_not_paused`) and
  event-publishing patterns from `admin.rs`, so it is consistent with the rest
  of the Aegis ecosystem.
- `mint_asset` in `asset.rs` now enforces the cap; existing whitelist and role
  checks are unchanged.
- Tests covering the default state, 2-step flow, cancel, no-op/negative
  rejection, and the lower-cap edge case live in `src/test.rs`.
