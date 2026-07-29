# Contract Error Code Standard

This document defines the standardized error codes exposed by the Aegis RWA
Protocol contracts, and how SDKs and dashboards should map them into
user-facing messages.

## Why

Before this standard, every failure path reverted with a plain `assert!`
string (e.g. `"Unauthorized: only admin can assign roles"`). Those strings
are not part of the contract's ABI, are not guaranteed to stay stable, and
Soroban does not return them to callers at all — only a generic host trap
was observable off-chain. SDKs and dashboards had nothing reliable to match
on, and every consumer had to guess at what went wrong from context alone.

Contracts now revert with `Error(Contract, #<code>)`, where `<code>` is one
of the stable, numbered variants of the `Error` enum defined in
[`src/errors.rs`](../src/errors.rs). SDKs should match on the numeric code,
never on a message string.

## Ranges

Codes are grouped into category ranges spaced 1000 apart, so new error
variants can be added to a category without renumbering any other category.

| Range       | Category               | Notes                                              |
|-------------|-------------------------|-----------------------------------------------------|
| 1000–1999   | Configuration            | Contract setup / one-time configuration failures    |
| 2000–2999   | Storage                  | Missing or inconsistent on-chain state              |
| 3000–3999   | Admin & Authorization     | Role checks, admin transfer, pause/unpause           |
| 4000–4999   | Compliance                | Lifecycle status checks on transfer/mint participants, and lifecycle transition validation |
| 5000–5999   | Minting & Transfers       | Amount and balance validation                        |
| 6000–6999   | Asset Metadata            | Asset lifecycle/metadata validation                  |
| 7000–7999   | Transfer Restrictions     | Granular reasons a transfer or mint was blocked      |

## Codes

| Code | Name                     | Category       | Meaning                                                              |
|------|--------------------------|----------------|-----------------------------------------------------------------------|
| 1000 | `AlreadyInitialized`     | Configuration  | `initialize` was called on a contract that already has an admin set. |
| 2000 | `NotInitialized`         | Storage        | No admin is set in storage; the contract was never initialized.      |
| 2001 | `NoPendingAdminTransfer` | Storage        | `accept_admin` was called with no `transfer_admin` in flight.        |
| 3000 | `Unauthorized`           | Admin/Auth     | Caller lacks the role or admin rights required for this call.        |
| 3001 | `CannotAssignAdminRole`  | Admin/Auth     | `set_role` was called with `Role::Admin`; use `transfer_admin`.      |
| 3002 | `NoRoleToRevoke`         | Admin/Auth     | `remove_role` target already has `Role::None`.                       |
| 3003 | `NotPendingCandidate`    | Admin/Auth     | `accept_admin` caller is not the recorded candidate.                 |
| 3004 | `ContractPaused`         | Admin/Auth     | The operation is blocked because the contract is paused.             |
| 3005 | `AlreadyPaused`          | Admin/Auth     | `pause` was called while already paused.                             |
| 3006 | `NotPaused`              | Admin/Auth     | `unpause` was called while not paused.                                |
| 3007 | `IssuanceDutyConflict`   | Admin/Auth     | Issuer separation is enforced and the caller holds both the compliance and issuance duties. See [`issuer-role-separation.md`](issuer-role-separation.md). |
| 3008 | `SelfIssuanceForbidden`  | Admin/Auth     | Issuer separation is enforced and the caller is the recipient of its own issuance. |
| 3009 | `IssuanceApproverConflict` | Admin/Auth   | Issuer separation is enforced and the caller approved the recipient's compliance. |
| 4000 | `SenderNotWhitelisted`   | Compliance     | The sending address has no current clearance (`Unknown` or `Revoked`). |
| 4001 | `ReceiverNotWhitelisted` | Compliance     | The receiving address has no current clearance (`Unknown` or `Revoked`). |
| 4002 | `SenderBlocked`          | Compliance     | The sending address is `Blocked` — sanctioned or frozen.             |
| 4003 | `ReceiverBlocked`        | Compliance     | The receiving address is `Blocked` — sanctioned or frozen.           |
| 4004 | `SenderCompliancePending`| Compliance     | The sending address is `Pending` — KYC submitted, not yet cleared.   |
| 4005 | `ReceiverCompliancePending` | Compliance  | The receiving address is `Pending` — KYC submitted, not yet cleared. |
| 4006 | `InvalidComplianceTransition` | Compliance | The requested compliance status transition is not permitted by the lifecycle state machine. |
| 4007 | `ComplianceStatusUnchanged` | Compliance  | The requested status equals the address's current status — no transition to apply. |
| 5000 | `InvalidAmount`          | Minting/Transfer | The requested amount was not strictly greater than zero.           |
| 5001 | `InsufficientBalance`    | Minting/Transfer | The sender's balance cannot cover the requested transfer amount.   |

| 6000 | `AssetNotActive`         | Asset Metadata | **Reserved.** Superseded by `7000`–`7002`; no longer emitted by `transfer`/`mint_asset`. |
| 6001 | `InvalidAssetStatusTransition` | Asset Metadata | The requested lifecycle transition is not permitted.         |
| 6002 | `AssetMetadataUpdateBlocked`   | Asset Metadata | Metadata cannot be updated in a terminal lifecycle status.   |
| 7000 | `AssetPausedRestriction` | Transfer Restriction | Asset lifecycle status is `Paused` — movements temporarily suspended. |
| 7001 | `AssetRetiredRestriction`| Transfer Restriction | Asset lifecycle status is `Retired` — terminal, never resumes. |
| 7002 | `AssetBlockedRestriction`| Transfer Restriction | Asset lifecycle status is `Blocked` — administrative hold.     |
| 7003 | `HoldingCapExceeded`     | Transfer Restriction | Crediting the recipient would breach the per-investor holding cap. |
| 7004 | `SupplyCapExceeded`      | Transfer Restriction | The mint would breach the global supply cap.                   |

Codes `3000`, `3004`, `4000`, `4001`, `5000`, `5001`, and `7000`–`7004` are the
**transfer restriction** surface: each maps 1:1 onto a `RestrictionReason` and
has recommended user-facing copy in
[Transfer Restriction Reason Codes](transfer-restrictions.md).

| 5002 | `SupplyCapExceeded`     | Minting/Transfer | Minting would exceed the active global supply cap.                  |
| 5003 | `HoldingCapExceeded`    | Minting/Transfer | The investor's balance would exceed the active holding cap.         |


## SDK mapping guidance

1. **Match on the numeric code, not the message.** Soroban surfaces
   contract errors as `Error(Contract, #<code>)`. Parse `<code>` and look it
   up in the table above (or the generated `Error` enum bindings) — do not
   pattern-match on any string.
2. **Group by range for coarse-grained handling.** A client that only needs
   to know "is this a compliance problem or an authorization problem" can
   integer-divide the code by 1000 to get the category, without enumerating
   every individual variant.
3. **Recommended user-facing messages:**
   - `3000 Unauthorized` / `3001`–`3003` → "You don't have permission to
     perform this action." (Do not expose role internals to end users.)
   - `3004`–`3006` (pause-related) → "This contract is currently paused for
     maintenance. Please try again later."
   - `3007`–`3009` (issuer separation) → "This issuance requires a different
     authorized key." The action is not retryable by the same caller; route
     the operator to a segregated issuance key rather than inviting a retry.
     See [`issuer-role-separation.md`](issuer-role-separation.md).
   - `4000`/`4001` → "This address has not completed compliance
     verification." Prompt the user toward the whitelist/KYC flow rather
     than showing a raw error.
   - `4002`/`4003` → "This address is restricted." **Never invite a retry
     or a KYC re-submission** — a `Blocked` address is under an enforcement
     freeze that only the admin can lift. Direct the user to support.
   - `4004`/`4005` → "Compliance review is still in progress." No user
     action is available; show the expected timeline rather than a retry.
   - `4006` → An admin UI attempted an illegal lifecycle transition. Drive
     the UI from `get_allowed_transitions_for` so this is unreachable.
   - `4007` → The address is already in the requested state; treat as a
     benign no-op rather than a failure.
   - `5000` → "Enter an amount greater than zero."
   - `5001` → "Insufficient balance for this transfer."

   - `7000` → "Transfers of this asset are temporarily suspended." (retryable)
   - `7001` → "This asset has been retired and can no longer be
     transferred." This is terminal — do **not** offer a retry.
   - `7002` → "Transfers of this asset are on hold pending issuer review."
   - `7003`/`7004` → "This would exceed the permitted holding / maximum
     supply." Show the remaining capacity alongside the message.

   - `5002` → "This mint exceeds the configured maximum supply cap."
   - `5003` → "This action would exceed the investor's permitted holding limit."

   - `1000`/`2000`/`2001` → these indicate integration or environment bugs
     (calling the contract in the wrong state), not user error. Log them
     and surface a generic "Something went wrong" message rather than
     asking the user to retry.
4. **Unknown codes must fail safe.** Any code not in this table (including
   future additions in the `6000` and `7000` ranges) should render as a
   generic "Transaction failed" message rather than crashing the client.
   Treat the table as additive/versioned — new codes may appear in future
   contract versions without any existing code changing meaning.
5. **Dashboards** should surface the raw numeric code alongside the mapped
   message (e.g. in an expandable "details" section) so support staff can
   cross-reference this document without needing contract source access.

## Blocked transfers

For the specific question "why was this transfer or mint rejected?", see
[Transfer Restriction Reason Codes](transfer-restrictions.md). It defines the
`RestrictionReason` enum, the pre-flight `check_transfer_restriction` /
`check_mint_restriction` entrypoints, check-order precedence, and the full
client mapping table. Because Soroban discards events from reverted
invocations, the numeric error code is the **only** off-chain-observable signal
for a blocked transfer.

## Adding a new error

- Pick the range matching the failure's category. If no range fits, open an
  issue to reserve a new one rather than repurposing an existing range.
- Never reuse or renumber an existing code — clients may have it cached or
  hardcoded. Deprecated codes should be documented as reserved, not deleted.
- Add a test in `src/test.rs` asserting the exact `Error` variant for the
  new failure path (see existing tests for the `Err(Ok(Error::...))`
  pattern used with `try_*` client methods).
