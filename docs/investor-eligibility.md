# Investor Eligibility Read Helpers

This document describes the investor eligibility read helpers for the Aegis
RWA Contracts (issue #14). They are pure, read-only compositions of the
protocol's existing compliance, holding-restriction, and pause state — no new
business logic or storage is introduced.

> **Not legal or financial advice.** These helpers surface protocol-level
> mechanics only. Whether an investor is eligible under a real-world
> regulatory regime, offering document, or subscription agreement is outside
> the scope of the contract and must be determined by the asset issuer's
> compliance and legal functions.

## Why

Before this change, an SDK or dashboard that needed to answer "can this
investor receive, hold, or send assets right now?" had to make several
separate calls — `is_whitelisted`, `get_balance_of`, `get_holding_cap`,
`is_paused` — and combine them off-chain. Reading them separately risks the
values coming from *different* ledger states (e.g. a whitelist revocation
lands between two reads), and every consumer had to re-implement the same
combination logic. These helpers compose the existing checks server-side, in
a single call, against a single consistent ledger snapshot.

Neither helper reads or writes any new storage key, requires authorization,
or can revert the invocation — they are safe to call from a read-only RPC
simulation at any time, including while the contract is paused.

## `get_investor_eligibility(investor: Address) -> InvestorEligibility`

Returns an aggregated eligibility snapshot for a single address.

```rust
pub struct InvestorEligibility {
    pub whitelisted: bool,
    pub compliance_status: ComplianceStatus,
    pub contract_paused: bool,
    pub balance: i128,
    pub holding_cap: i128,
    pub remaining_capacity: Option<i128>,
    pub can_receive: bool,
    pub can_send: bool,
}
```

| Field | Meaning |
| --- | --- |
| `whitelisted` | Whether the address is compliance-**approved**. Derived: `true` only when `compliance_status == Approved`. |
| `compliance_status` | The full lifecycle state — `Unknown`, `Pending`, `Approved`, `Revoked`, or `Blocked`. Prefer this over `whitelisted`: it distinguishes "never onboarded" from "under review" from "sanctioned", which map to different user-facing actions. See [`compliance-lifecycle.md`](compliance-lifecycle.md). |
| `contract_paused` | Whether the contract is currently paused. When `true`, no mint or transfer can succeed, regardless of the other fields. |
| `balance` | The investor's current token balance — for portfolio display. |
| `holding_cap` | The active global per-investor holding cap. `0` means unrestricted (see [`docs/investor-holding-restrictions.md`](investor-holding-restrictions.md)). |
| `remaining_capacity` | `holding_cap - balance`, floored at `0`. `None` when the holding cap is unrestricted (`0`) — there is no ceiling to compute headroom against. |
| `can_receive` | `whitelisted && !contract_paused && (holding_cap == 0 \|\| balance < holding_cap)` — true if the investor could currently receive at least `1` unit via mint or transfer. |
| `can_send` | `whitelisted && !contract_paused && balance > 0` — true if the investor could currently send at least `1` unit. |

## `check_transfer_eligibility(from: Address, to: Address, amount: i128) -> bool`

Returns whether a transfer of `amount` from `from` to `to` would currently
pass every check `transfer()` performs, without submitting or simulating a
mutating transaction:

1. `amount > 0`
2. Contract is not paused
3. The asset lifecycle status is `Active`
4. `from`'s compliance status is `Approved`
5. `to`'s compliance status is `Approved`
6. `from`'s balance is `>= amount`
7. `to`'s resulting balance (`balance + amount`) does not exceed the active holding cap (skipped when the cap is `0`)

This mirrors the exact order and conditions enforced in `asset.rs::transfer`,
so a `true` result and a subsequent `transfer` call are checking the same
invariants. It does **not** guarantee the subsequent `transfer` will succeed
— see [Point-in-time caveat](#point-in-time-caveat) below.

## Point-in-time caveat

Both helpers reflect ledger state at the moment they are called (or
simulated). Balances, whitelist membership, the holding cap, and pause state
can all change between a read and a subsequent state-changing call — e.g. a
compliance officer revokes the investor's whitelist status, or another
transfer fills their remaining holding-cap headroom, in between. SDKs and
dashboards should treat these helpers as **UX/pre-flight signals** — to gate
a "Send" button, show an eligibility badge, or avoid submitting a transaction
that is obviously going to fail — not as a substitute for handling the
possible revert codes documented in
[`docs/error-codes.md`](error-codes.md) (`3004`, `4000`, `4001`) on the
actual `mint_asset`/`transfer` call.

## SDK and dashboard usage

- **Portfolio view**: call `get_investor_eligibility(investor)` for the
  connected wallet address; render `balance` and, if `holding_cap > 0`, a
  "X of Y capacity used" indicator from `remaining_capacity`.
- **Transfer/send UX**: call `check_transfer_eligibility(from, to, amount)`
  before enabling a "Send" button or building a transaction, to short-circuit
  an obviously-failing transfer with a friendly message instead of a generic
  transaction failure. Still handle the revert if ledger state changes before
  submission.
- **Compliance dashboards**: call `get_investor_eligibility(investor)` per
  row in an investor list to show whitelist and holding-cap status without
  needing three separate RPC round-trips per row.
- Both functions are ordinary read calls — invoke them the same way as
  `get_balance_of` / `is_whitelisted` via `soroban contract invoke` (read-only,
  no signing) or the generated SDK client's simulate-only call path. No
  authorization/signature is required.

## Compatibility

- Reuses existing internal helpers with no behavior change to any
  state-changing function: `compliance::get_compliance_status`,
  `compliance::is_whitelisted`, `asset::get_asset_status_internal`,
  `holding::get_holding_cap`, `admin::is_paused`, and the same
  `DataKey::Balance` read as `get_balance_of`.
- Introduces no new storage keys, no new error codes, and no new events —
  pure reads have nothing to emit or fail.
- Tests covering the default (ineligible) state, an eligible whitelisted
  holder, holding-cap headroom, the paused state, and every
  `check_transfer_eligibility` false-case live in `src/test.rs`.
