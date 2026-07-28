# Investor Holding Restriction Checks

This document describes the investor holding restriction checks for the Aegis
RWA Contracts (issue #33). It is part of the protocol's compliance control
surface and is intended to be safe for RWA investor-protection use cases.

> **Not legal or financial advice.** This is protocol-level mechanics only.
> How a holding cap maps to a real-world investor limit, regulatory
> concentration rule, or legal agreement is outside the scope of the contract
> and must be determined by the asset issuer's compliance and legal functions.

## Data model

The contract enforces a single **global per-investor holding cap** (the Aegis
contract models one token per deployment). Two storage keys back it:

| Key | Meaning |
| --- | --- |
| `HoldingCap` | The currently active cap on a single address's balance. A value of `0` means **no restriction** (any whitelisted balance is allowed). |
| `HoldingCapCandidate` | A pending proposed cap awaiting 2-step acceptance. Absent when no proposal is outstanding. |

## Governance model: 2-step amendment

Changing the holding cap is a sensitive, compliance-relevant action. It follows
the same safe 2-step pattern used for the supply cap and admin transfer:

1. **Propose** — `propose_holding_cap(new_cap)` records the candidate. The
   active cap is **not** changed yet. Only the supreme admin can propose, and
   only when the contract is not paused. Proposing the same value as the active
   cap (a no-op) is rejected; negative values are rejected.
2. **Accept** — `accept_holding_cap()` promotes the candidate to the active cap
   and clears the proposal slot. Only the supreme admin can accept.
3. **Cancel** — `cancel_holding_cap_proposal()` discards an outstanding
   proposal without applying it. Only the supreme admin can cancel.

Both steps emit events (`holding_cap_proposed`, `holding_cap_amended`) so the
amendment is auditable off-chain.

## Enforcement

`enforce_holding_cap(address, incoming)` is called **before** crediting a
receiver, in both `mint_asset` and `transfer`. It computes the resulting
balance (`current_balance + incoming`) and reverts when it would exceed the
active cap (only when the cap is `> 0`). The check runs for every caller,
because the cap is a protocol-level investor-protection invariant.

- If the active cap is `0`, neither minting nor transfers are blocked by the
  holding cap (the compliance whitelist still applies).
- If the cap is `> 0`, any credit that would push a holder's balance above the
  cap reverts.

## Edge cases and failure states

| Case | Behaviour |
| --- | --- |
| No cap set (default) | `get_holding_cap()` returns `0`; holdings are unrestricted (whitelist still enforced). |
| Propose by non-admin | Reverts (`Unauthorized: only admin can propose a holding cap`). |
| Accept with no proposal | Reverts (`No pending holding cap proposal to accept`). |
| Cancel with no proposal | Reverts (`No pending holding cap proposal to cancel`). |
| Proposed cap equals active cap | Reverts (`Proposed cap equals the active cap — no change requested`). |
| Negative proposed cap | Reverts (`Holding cap must be non-negative`). |
| Mint would exceed a holder's cap | Reverts (`Transfer would exceed the investor holding cap`). |
| Transfer would exceed receiver's cap | Reverts (same message). |
| Cap lowered below a holder's current balance | Allowed. The holder's existing balance is **not** clawed back; they simply cannot receive further tokens until their balance falls or the cap is raised. |
| Contract paused | All holding-cap governance calls and `mint_asset`/`transfer` revert. |

## Why 2-step (and not immediate)?

An immediate cap change — especially one set too low, or set to `0` after a
high cap — could freeze investor balances or, conversely, remove a safeguard.
Requiring an explicit accept step gives the admin a deliberate checkpoint and a
window to cancel a mistaken proposal, mirroring the governance model already
used for supply caps (`supply-cap-governance.md`) and admin transfer.

## Compatibility

- Reuses the existing RBAC (`get_admin`, `require_not_paused`) and event
  patterns from `admin.rs`, consistent with the rest of the Aegis ecosystem.
- `mint_asset` and `transfer` in `asset.rs` now enforce the holding cap on the
  receiver; existing whitelist, role, and (where also enabled) supply-cap
  checks are unchanged.
- Tests covering the default state, mint/transfer enforcement, and the 2-step
  governance flow live in `src/test.rs`.
