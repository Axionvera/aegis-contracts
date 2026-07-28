# Transfer Restriction Reason Codes

This document defines the **restriction reason codes** the Aegis RWA contracts
produce when an asset movement (a `transfer` or a `mint_asset`) is blocked, and
the exact mapping SDK and dashboard clients must implement to turn them into
user-facing explanations.

It is the transfer-specific companion to the
[Contract Error Code Standard](error-codes.md): that document catalogues *all*
contract errors; this one covers the subset that means "your movement was
blocked, and here is why".

---

## 1. Why

A blocked transfer used to be difficult to explain off-chain:

* Three genuinely different asset states — **paused**, **retired**, and
  **blocked** — all collapsed into one code, `6000 AssetNotActive`. A client
  could not tell "try again in an hour" from "this asset is retired forever".
* Cap breaches (`holding cap`, `supply cap`) reverted through bare `assert!`
  string panics. Soroban does **not** return panic messages to callers, so the
  client saw only a generic host trap with no contract error code at all.
* There was no way to ask the contract *why* a transfer would fail before
  signing. `check_transfer_eligibility` returned a bare `bool` — enough to
  disable a button, not enough to label it.

Soroban discards events emitted by a reverted invocation, so a "TransferBlocked"
event is structurally impossible (see `transfer_restriction_events` in
[capabilities.md](capabilities.md)). **The error code is the only
off-chain-observable signal for a blocked transfer**, which is precisely why it
must be specific.

## 2. The `RestrictionReason` enum

Defined in [`src/restrictions.rs`](../src/restrictions.rs). Every blocking
condition on a movement path resolves to exactly one variant.

| Variant                 | Error code | Category      | Terminal? | Meaning                                                            |
|-------------------------|-----------:|---------------|:---------:|--------------------------------------------------------------------|
| `None`                  |      *(0)* | —             |     —     | No restriction — the movement passes every check right now.        |
| `UnauthorizedOperation` |     `3000` | Authorization |     no    | The caller lacks the role required for the operation.              |
| `ContractPaused`        |     `3004` | Protocol      |     no    | The contract is under a global emergency pause.                    |
| `SenderNotCompliant`    |     `4000` | Compliance    |     no    | The **sending** address is not on the compliance whitelist.        |
| `RecipientNotCompliant` |     `4001` | Compliance    |     no    | The **receiving** address is not on the compliance whitelist.      |
| `InvalidAmount`         |     `5000` | Input         |     no    | The amount is not strictly greater than zero.                      |
| `InsufficientBalance`   |     `5001` | Balance       |     no    | The sender's balance cannot cover the amount.                      |
| `AssetPaused`           |     `7000` | Asset state   |     no    | Asset lifecycle status is `Paused` — temporary, may resume.        |
| `AssetRetired`          |     `7001` | Asset state   |  **yes**  | Asset lifecycle status is `Retired` — terminal, never resumes.     |
| `AssetBlocked`          |     `7002` | Asset state   |     no    | Asset lifecycle status is `Blocked` — administrative hold.         |
| `HoldingCapExceeded`    |     `7003` | Cap           |     no    | Crediting the recipient would breach the per-investor holding cap. |
| `SupplyCapExceeded`     |     `7004` | Cap           |     no    | The mint would breach the global supply cap.                       |

`None` is the only non-blocking variant and the only one with no error code;
`get_restriction_code` returns `0` for it.

**Terminal** means no retry at any future ledger, under any state, can succeed.
Only `AssetRetired` is terminal — `Retired` is a terminal lifecycle state with
no outbound transition (see [`src/asset.rs`](../src/asset.rs)). Clients must
suppress retry affordances for terminal reasons and may offer them otherwise.

### New error codes: the 7000 range

The `7000` range is newly reserved for **transfer / movement restrictions**.
Existing codes are unchanged and keep their meanings — nothing was renumbered.

| Code   | Name                       | Replaces                                          |
|--------|----------------------------|---------------------------------------------------|
| `7000` | `AssetPausedRestriction`   | `6000 AssetNotActive` (when status is `Paused`)   |
| `7001` | `AssetRetiredRestriction`  | `6000 AssetNotActive` (when status is `Retired`)  |
| `7002` | `AssetBlockedRestriction`  | `6000 AssetNotActive` (when status is `Blocked`)  |
| `7003` | `HoldingCapExceeded`       | an untyped `assert!` host panic                    |
| `7004` | `SupplyCapExceeded`        | an untyped `assert!` host panic                    |

> **Migration note.** `6000 AssetNotActive` is **retained as a reserved code**
> for compatibility — it is never deleted and never reused — but `transfer` and
> `mint_asset` no longer emit it. Clients that special-cased `6000` should map
> `7000`/`7001`/`7002` instead; a client that has not been updated will fall
> through to its "unknown code" branch, which per the error-code standard must
> already render a safe generic message.

## 3. Contract API

Four entrypoints, all pure reads: no storage writes, no events, no
authorization required. They remain callable while the contract is paused,
before `initialize`, and against terminal asset states, and they never panic.

| Entrypoint                                              | Returns             | Purpose                                                 |
|---------------------------------------------------------|---------------------|----------------------------------------------------------|
| `check_transfer_restriction(from, to, amount)`           | `RestrictionReason` | Why a transfer would be blocked right now.               |
| `check_mint_restriction(caller, to, amount)`             | `RestrictionReason` | Why a mint would be blocked right now.                   |
| `get_restriction_code(reason)`                           | `u32`               | The numeric error code a reason reverts with (`0` = none).|
| `get_restriction_schema_version()`                       | `u32`               | Schema version of the reason enumeration (currently `1`).|

`check_transfer_eligibility` is unchanged and still returns a `bool`; it is now
implemented in terms of the same evaluator, so the two can never disagree:

```
check_transfer_eligibility(f, t, a) == !check_transfer_restriction(f, t, a).is_blocked()
```

This invariant is asserted in `src/test.rs`.

## 4. Check order and precedence

The evaluators run their checks in **exactly** the order the state-changing
entrypoint does, so a pre-flight reason always matches the eventual revert.
When several conditions apply at once, the first one in the list wins.

**`transfer`:**

1. Global contract pause → `ContractPaused`
2. Amount validation → `InvalidAmount`
3. Asset lifecycle status → `AssetPaused` / `AssetRetired` / `AssetBlocked`
4. Sender compliance → `SenderNotCompliant`
5. Recipient compliance → `RecipientNotCompliant`
6. Recipient holding cap → `HoldingCapExceeded`
7. Sender balance → `InsufficientBalance`

**`mint_asset`:**

1. Global contract pause → `ContractPaused`
2. Caller authorization → `UnauthorizedOperation`
3. Amount validation → `InvalidAmount`
4. Asset lifecycle status → `AssetPaused` / `AssetRetired` / `AssetBlocked`
5. Recipient compliance → `RecipientNotCompliant`
6. Global supply cap → `SupplyCapExceeded`
7. Recipient holding cap → `HoldingCapExceeded`

Two consequences clients must design for:

* **The response is the *first* blocking reason, not the only one.** If both
  parties fail KYC, only `SenderNotCompliant` is reported. After the user fixes
  it, re-query — a second reason may surface.
* **A global pause outranks asset state.** A paused contract holding a retired
  asset reports `ContractPaused`, not `AssetRetired`.

## 5. SDK / dashboard mapping

### 5.1 Rules

1. **Match on the numeric code, never on a message string.** Soroban surfaces
   contract failures as `Error(Contract, #<code>)`. Panic messages are not
   returned to callers at all.
2. **Derive the mapping from the chain where possible.** Call
   `get_restriction_code(reason)` rather than hardcoding the table, so an SDK
   pinned to an older version cannot silently mis-label a code.
3. **Check `get_restriction_schema_version()`** before assuming you know every
   variant. Variants are append-only; a higher version than you know about
   means unknown reasons may appear.
4. **Unknown codes must fail safe** — render a generic "Transaction failed"
   rather than crashing or guessing.
5. **Pre-flight, then still handle the revert.** Every reason is a
   point-in-time verdict; whitelist membership, caps, balances, pause state,
   and asset status can all change between the read and submission.
6. **Never surface role internals.** `UnauthorizedOperation` should read as a
   permissions message, not an RBAC dump.

### 5.2 Recommended user-facing copy

| Reason                  | Suggested message                                                                 | Suggested action                        |
|-------------------------|------------------------------------------------------------------------------------|-----------------------------------------|
| `UnauthorizedOperation` | "You don't have permission to perform this action."                                | Hide/disable the control.               |
| `ContractPaused`        | "Transfers are temporarily paused for maintenance. Please try again later."        | Offer retry; show status banner.        |
| `AssetPaused`           | "Transfers of this asset are temporarily suspended by the issuer."                 | Offer retry; link to asset status.      |
| `AssetRetired`          | "This asset has been retired and can no longer be transferred."                    | **No retry.** Link to holdings history. |
| `AssetBlocked`          | "Transfers of this asset are on hold pending issuer review."                       | Link to issuer contact/support.         |
| `SenderNotCompliant`    | "Your account hasn't completed compliance verification yet."                       | Deep-link to the KYC flow.              |
| `RecipientNotCompliant` | "The recipient's address hasn't completed compliance verification."                | Prompt to invite/verify the recipient.  |
| `InvalidAmount`         | "Enter an amount greater than zero."                                               | Inline field validation.                |
| `InsufficientBalance`   | "Insufficient balance for this transfer."                                          | Show available balance; offer "max".    |
| `HoldingCapExceeded`    | "This transfer would exceed the recipient's maximum permitted holding."            | Show remaining capacity.                |
| `SupplyCapExceeded`     | "This issuance would exceed the asset's maximum supply."                           | Show remaining headroom.                |

### 5.3 Reference client mapping

```ts
// Codes are stable and additive — never renumbered, never reused.
export const RESTRICTION_CODES = {
  3000: { reason: "UnauthorizedOperation", terminal: false, retryable: false },
  3004: { reason: "ContractPaused",        terminal: false, retryable: true  },
  4000: { reason: "SenderNotCompliant",    terminal: false, retryable: false },
  4001: { reason: "RecipientNotCompliant", terminal: false, retryable: false },
  5000: { reason: "InvalidAmount",         terminal: false, retryable: false },
  5001: { reason: "InsufficientBalance",   terminal: false, retryable: false },
  7000: { reason: "AssetPaused",           terminal: false, retryable: true  },
  7001: { reason: "AssetRetired",          terminal: true,  retryable: false },
  7002: { reason: "AssetBlocked",          terminal: false, retryable: false },
  7003: { reason: "HoldingCapExceeded",    terminal: false, retryable: false },
  7004: { reason: "SupplyCapExceeded",     terminal: false, retryable: false },
} as const;

export function explainTransferFailure(code: number) {
  const entry = RESTRICTION_CODES[code as keyof typeof RESTRICTION_CODES];
  if (!entry) {
    // Unknown or non-restriction error (config/storage/governance) — fail safe.
    return { message: "Transaction failed.", retryable: false, terminal: false };
  }
  return { ...entry, message: MESSAGES[entry.reason] };
}
```

Pre-flight before asking the user to sign:

```ts
const reason = await client.check_transfer_restriction({ from, to, amount });
if (reason.tag !== "None") {
  const code = await client.get_restriction_code({ reason });
  showBlocked(explainTransferFailure(code));
  return;              // don't build a transaction we know will revert
}
await client.transfer({ from, to, amount });
```

### 5.4 What is *not* a restriction

Errors outside this surface — `1000 AlreadyInitialized`, `2000 NotInitialized`,
`2001 NoPendingAdminTransfer`, `3001`–`3003`, `3005`/`3006`,
`6001 InvalidAssetStatusTransition`, `6002 AssetMetadataUpdateBlocked` — are
integration or environment faults, **not** transfer restrictions. The
`reason_for_error` helper returns `None` for them, and clients must not render
them as "your transfer was blocked because…". Log them and show a generic
error.

## 6. Capability discovery

Clients can feature-detect this whole surface before using it:

* `get_capabilities().transfers.transfer_restriction_reasons` → `Supported`
* `supports_capability(Symbol("transfer_restriction_reasons"))` → `Supported`

Deployments that report `Unsupported` predate this feature and will still emit
the legacy `6000 AssetNotActive` and untyped cap panics. The capability schema
version was bumped to `2` for the added field.

## 7. Stability contract

* Reason variants and their numeric codes are **append-only**. Never remove,
  reorder, renumber, or repurpose one.
* Adding a variant requires bumping `RESTRICTION_SCHEMA_VERSION` in
  [`src/restrictions.rs`](../src/restrictions.rs), adding the row to the table
  above and to [error-codes.md](error-codes.md), and extending the round-trip
  test in `src/test.rs`.
* The reason ⇄ code mapping must stay **total and bijective** over blocking
  reasons. `test_restriction_reason_code_mapping_is_total_and_round_trips`
  enforces this: a new reason without a code fails the build's test gate.

## 8. Test coverage

Asserted in [`src/test.rs`](../src/test.rs):

| Test                                                                | Guarantee                                                         |
|---------------------------------------------------------------------|--------------------------------------------------------------------|
| `test_unrestricted_transfer_reports_no_reason`                       | Clean path reports `None` / code `0`.                             |
| `test_restriction_reason_non_compliant_sender`                       | Non-compliant sender → `SenderNotCompliant` / `4000`.             |
| `test_restriction_reason_non_compliant_recipient`                    | Non-compliant recipient → `RecipientNotCompliant` / `4001`.       |
| `test_restriction_reason_sender_checked_before_recipient`            | Deterministic precedence when both fail.                          |
| `test_restriction_reason_paused_asset`                               | Paused asset → `7000`, non-terminal, lifts on `Active`.           |
| `test_restriction_reason_retired_asset`                              | Retired asset → `7001` and is terminal.                           |
| `test_restriction_reason_blocked_asset_is_distinct_from_paused_and_retired` | The three asset states never collapse to one code.        |
| `test_restriction_reason_contract_pause_outranks_asset_state`        | Documented precedence holds.                                      |
| `test_restriction_reason_unauthorised_operation_on_mint`             | Unauthorised caller → `UnauthorizedOperation` / `3000`.           |
| `test_restriction_reason_invalid_amount_and_insufficient_balance`    | Amount and balance reasons.                                       |
| `test_restriction_reason_holding_cap_exceeded_on_transfer`           | Holding cap → typed `7003` instead of a host panic.               |
| `test_restriction_reason_supply_cap_exceeded_on_mint`                | Supply cap → typed `7004` instead of a host panic.                |
| `test_restriction_reasons_on_mint_cover_compliance_and_asset_state`  | Mint path reasons match its reverts.                              |
| `test_restriction_reason_code_mapping_is_total_and_round_trips`      | Mapping is total, round-trips, and excludes non-restrictions.     |
| `test_restriction_checks_are_pure_reads_and_survive_paused_state`    | Reads never mutate and work while paused.                         |
| `test_restriction_checks_never_panic_on_uninitialized_contract`      | Reads fail safe with no admin in storage.                         |
| `test_blocked_transfers_leave_no_state_change_and_emit_no_event`     | Blocked transfers are atomic and event-free.                      |
| `test_restriction_reason_agrees_with_check_transfer_eligibility`     | Boolean and reason verdicts never disagree.                       |
| `test_investor_eligibility_snapshot_agrees_with_restriction_reasons` | Snapshot and reason views stay consistent.                        |

---

**See also:** [Contract Error Code Standard](error-codes.md) ·
[Investor Eligibility Read Helpers](investor-eligibility.md) ·
[Investor Holding Restriction Checks](investor-holding-restrictions.md) ·
[Contract Capability Flags](capabilities.md) · [Events](events.md)
