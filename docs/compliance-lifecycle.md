# Compliance Status Lifecycle

This document specifies the investor compliance lifecycle enforced by the
Aegis RWA Contracts: the five states an address can be in, the transitions
allowed between them, who may authorize each transition, how minting and
transfers consume the state, and the events emitted on every change.

> **Not legal or financial advice.** These states model protocol-level
> mechanics only. Whether an investor is eligible under a real-world
> regulatory regime is determined off-chain by the issuer's compliance and
> legal functions — see
> [`legal-boundary-disclaimer.md`](legal-boundary-disclaimer.md). The contract
> records the *outcome* of that determination; it does not make it.

## Why

Before this change, compliance was a single boolean:
`DataKey::Whitelist(Address)` was either present or absent. That is too
coarse for a realistic RWA workflow, and it conflated four materially
different situations into one "not whitelisted" answer:

| Real-world situation | Old representation | Correct client response |
| --- | --- | --- |
| Investor has never onboarded | absent | "Start KYC." |
| KYC submitted, under review | absent | "We're reviewing your application." |
| Clearance lapsed / investor offboarded | absent | "Re-verify your documents." |
| Sanctioned or frozen by enforcement | absent | **Do not invite a retry.** Escalate. |

A dashboard could not tell these apart, so it could only render a generic
"not verified" message — including to a sanctioned address, which is exactly
the case where inviting a re-submission is inappropriate. Worse, the boolean
had no notion of an *illegal* change: any writer could flip a sanctioned
address straight back to cleared in a single call, with nothing in the
contract to stop it and no record of what the previous state had been.

The lifecycle makes the state explicit, makes the legal changes between
states a contract-enforced invariant, and emits the before/after pair on
every committed change.

## States

```rust
pub enum ComplianceStatus {
    Unknown,   // default — no compliance record exists
    Pending,   // KYC/AML review started, not yet cleared
    Approved,  // cleared — the only permissive state
    Revoked,   // clearance withdrawn
    Blocked,   // sanctioned / frozen by enforcement
}
```

| State | Can receive | Can send | Meaning |
| --- | :---: | :---: | --- |
| `Unknown` | ✗ | ✗ | No record exists. The default for every address the registry has never seen. |
| `Pending` | ✗ | ✗ | Known to the registry, review in flight. **Fails closed** — an in-flight review never grants access. |
| `Approved` | ✓ | ✓ | Compliance cleared. The only state in which an address may hold, send, or receive. |
| `Revoked` | ✗ | ✗ | Clearance withdrawn (expired documents, lapsed attestation, offboarding). Existing balance is retained but frozen. |
| `Blocked` | ✗ | ✗ | Enforcement freeze. Like `Revoked` but escalated: only the supreme admin can lift it, and only into re-review. |

Two properties are worth stating explicitly:

* **`Unknown` is the fail-closed default.** An address with no storage entry
  reads as `Unknown` and is permitted nothing. A failed or partial write can
  never accidentally grant access.
* **Non-approved states freeze a balance; they never destroy it.** A blocked
  or revoked holder keeps their units — they simply cannot move them, and
  nobody can credit them further. There is no burn or seizure path (see
  [`capabilities.md`](capabilities.md), `minting.burning` is `Unsupported`).

## Transition matrix

| From ↓ / To → | `Unknown` | `Pending` | `Approved` | `Revoked` | `Blocked` |
| --- | :---: | :---: | :---: | :---: | :---: |
| **`Unknown`**  | — | ✓ | ✓ | ✗ | ✓ |
| **`Pending`**  | ✗ | — | ✓ | ✓ | ✓ |
| **`Approved`** | ✗ | ✓ | — | ✓ | ✓ |
| **`Revoked`**  | ✗ | ✓ | ✓ | — | ✓ |
| **`Blocked`**  | ✗ | ✓ *(admin only)* | ✗ | ✗ | — |

Three global rules shape the matrix:

1. **No self-transitions.** `from == to` reverts with
   `ComplianceStatusUnchanged` (4007) rather than succeeding as a silent
   no-op, so an event is never emitted for a change that did not happen.
2. **`Unknown` is never a target.** Compliance history cannot be erased.
   Offboarding an investor is `Revoked`, not a reset to "never seen" — an
   auditor must always be able to tell the two apart.
3. **`Blocked` is a quarantine with exactly one exit.** A sanctioned address
   can only be moved back into `Pending` (re-review), never directly to
   `Approved`. Lifting a freeze therefore *always* forces a fresh compliance
   review before assets can move again.

Rejected transitions revert with `InvalidComplianceTransition` (4006) and
leave state untouched.

### Why `Unknown → Revoked` is rejected

Nothing has ever been granted, so there is nothing to withdraw. Allowing it
would let an operator manufacture a "revoked" record for an address that
never onboarded, which misrepresents the audit trail.

### Why entering `Blocked` is broadly available but leaving it is not

Any compliance role can *impose* a freeze — sanctions screening is
time-sensitive and must not wait on the admin key. Only the supreme admin can
*lift* one. This mirrors the pause/unpause asymmetry in
[`admin-roles.md`](admin-roles.md): a compromised or coerced ComplianceOfficer
can inconvenience the protocol by over-freezing, but cannot release an address
that enforcement has frozen. See [`admin-misuse-risks.md`](admin-misuse-risks.md).

## Authorization

| Transition | Required authority |
| --- | --- |
| Any transition **from** `Blocked` | Supreme admin **only** (`Unauthorized` 3000 otherwise) |
| Every other transition | ComplianceOfficer, EmergencyOfficer, or Admin |

All lifecycle writes are blocked while the contract is paused
(`ContractPaused` 3004) — see [`emergency-pause.md`](emergency-pause.md).

## API

### Writes

* **`set_compliance_status(caller, user, new_status)`** — the canonical
  lifecycle entrypoint. Validates the transition against the matrix, enforces
  the authorization rules above, and emits `compliance_status_changed`.

  Reverts with `ComplianceStatusUnchanged` (4007) for a no-op,
  `InvalidComplianceTransition` (4006) for an illegal transition,
  `Unauthorized` (3000) for an unauthorized caller, and `ContractPaused`
  (3004) while paused.

* **`batch_set_compliance_status(caller, updates)`** - the atomic batch
  lifecycle entrypoint. Each update is a typed `ComplianceBatchUpdate`
  (`user`, `new_status`). The contract validates the whole batch before
  writing, then emits one `compliance_status_changed` event per address in
  input order.

  Empty batches return `0`. Duplicate users, illegal transitions, and
  order-dependent intent are rejected without partial writes. See
  [`compliance-batch-updates.md`](compliance-batch-updates.md).

* **`whitelist_user(admin, user)`** — legacy alias for a transition to
  `Approved`. Retained for backwards compatibility. Idempotent: re-approving
  an already-`Approved` address succeeds without emitting a lifecycle event.
  Reverts with `InvalidComplianceTransition` if the address is `Blocked` — a
  sanctions freeze cannot be lifted through the legacy path.

* **`revoke_whitelist(admin, user)`** — legacy alias for a transition to
  `Revoked`. Deliberately tolerant, matching the pre-lifecycle behaviour:
  revoking an `Unknown` address is a no-op rather than an error, and revoking
  a `Blocked` address leaves the stronger state intact rather than silently
  downgrading an enforcement freeze.

### Reads

All reads are pure: no storage writes, no events, no authorization required,
never revert — including before `initialize` and while paused.

* **`get_compliance_status(user) -> ComplianceStatus`** — the address's
  current state (`Unknown` when no record exists).
* **`is_compliance_transition_allowed(from, to) -> bool`** — pre-flight a
  transition without building a transaction.
* **`get_allowed_transitions(from) -> Vec<ComplianceStatus>`** — every state
  reachable from `from` in one step.
* **`get_allowed_transitions_for(user) -> Vec<ComplianceStatus>`** — the same,
  for the address's *current* state. Lets a dashboard render only the legal
  next actions instead of hardcoding the matrix and drifting out of sync.
* **`is_whitelisted(user) -> bool`** — retained and now **derived**: `true`
  only for `Approved`. Prefer `get_compliance_status`, which distinguishes the
  four non-approved states.

## Enforcement on minting and transfers

Both `mint_asset` and `transfer` consume the lifecycle state directly. Only
`Approved` permits movement; every other state maps to a distinct error code
so a client can respond appropriately instead of showing one generic message:

| Status | On mint / receive | On send |
| --- | --- | --- |
| `Approved` | permitted | permitted |
| `Unknown` | `ReceiverNotWhitelisted` (4001) | `SenderNotWhitelisted` (4000) |
| `Revoked` | `ReceiverNotWhitelisted` (4001) | `SenderNotWhitelisted` (4000) |
| `Pending` | `ReceiverCompliancePending` (4005) | `SenderCompliancePending` (4004) |
| `Blocked` | `ReceiverBlocked` (4003) | `SenderBlocked` (4002) |

`Unknown` and `Revoked` deliberately share a code: both mean "no current
clearance, KYC is the remedy", and the pre-lifecycle codes 4000/4001 keep
their exact meaning so existing integrations do not break.

In `transfer`, the **sender is checked before the receiver**, so when both
parties are ineligible the sender's status is the one reported.

`check_transfer_eligibility` mirrors these checks exactly (plus pause, asset
lifecycle status, holding cap, and balance), so a `true` result and a
subsequent `transfer` evaluate the same invariants. See
[`investor-eligibility.md`](investor-eligibility.md) for the point-in-time
caveat.

## Events

| Topic | Payload | Emitted by |
| --- | --- | --- |
| `compliance_status_changed` | `caller: Address`, `user: Address`, `previous_status: ComplianceStatus`, `new_status: ComplianceStatus` | `set_compliance_status`, and the legacy wrappers when they cause a real transition |
| `user_whitelisted` | `caller: Address`, `user: Address` | `whitelist_user` (legacy, always) |
| `whitelist_revoked` | `caller: Address`, `user: Address` | `revoke_whitelist` (legacy, always) |

`compliance_status_changed` is the **canonical signal for indexers**: it
carries both the previous and the new state, so a projection can be rebuilt
from the event stream alone without replaying every prior event to infer what
the address was before. The two legacy events are retained for backwards
compatibility and are emitted *in addition to* the lifecycle event.

Ordering within a single legacy call is: `compliance_status_changed` first,
then the legacy event.

As always in Soroban, a reverted invocation discards its events — a rejected
transition emits nothing. See [`events.md`](events.md).

## Storage

| Key | Value | Class | Notes |
| --- | --- | --- | --- |
| `ComplianceStatus(Address)` | `ComplianceStatus` | Persistent | **Source of truth.** Absent ⇒ `Unknown`. |
| `Whitelist(Address)` | `bool` | Persistent | **Derived mirror**, kept for backwards compatibility. Present and `true` iff status is `Approved`; removed otherwise. Never read as the source of truth. |

The mirror is written by a single helper (`write_status`) alongside every
lifecycle write, so the two keys cannot drift apart.

## Backwards compatibility

* `is_whitelisted` keeps its name, signature, and meaning ("may this address
  transact?"). It is now derived from the lifecycle.
* `whitelist_user` / `revoke_whitelist` keep their signatures and their events.
* Error codes 4000 and 4001 keep their exact meaning. The new codes
  (4002–4007) are additive, consistent with the append-only rule in
  [`error-codes.md`](error-codes.md).
* `InvestorEligibility` gains a `compliance_status` field; all existing fields
  are unchanged.
* `CAPABILITY_SCHEMA_VERSION` is bumped to `2` for the added capability fields
  and keys — see [`capabilities.md`](capabilities.md).

**One behaviour intentionally changed:** `whitelist_user` now *reverts* with
`InvalidComplianceTransition` on a `Blocked` address instead of silently
approving it. That is the point of the feature — a sanctions freeze must not
be liftable through the legacy path — and it cannot be triggered by any
address that predates this change, since no address could have been `Blocked`
before the lifecycle existed.

## Capability advertisement

A deployment advertises lifecycle support so clients can feature-gate without
probing:

| Capability key | Field | Value |
| --- | --- | --- |
| `compliance_lifecycle` | `compliance.lifecycle_states` | `Supported` |
| `compliance_transitions` | `compliance.lifecycle_transitions` | `Supported` |
| `compliance_lifecycle_events` | `events.compliance_lifecycle_events` | `Supported` |

`compliance.investor_tiers` remains `Unsupported`: the lifecycle models
compliance *state*, not investor *class*. Jurisdiction and accreditation
segmentation (Reg D vs. Reg S) is still off-chain only.

## SDK and dashboard guidance

1. **Render the state, not a boolean.** Map each status to a distinct badge
   and call to action:
   - `Unknown` → "Not verified" · *Start KYC*
   - `Pending` → "Under review" · *no action, show expected timeline*
   - `Approved` → "Verified"
   - `Revoked` → "Verification expired" · *Re-verify*
   - `Blocked` → "Restricted" · **no self-service remedy; contact support**
2. **Never invite a retry on `Blocked`.** This is the main reason the state
   exists. Treat 4002/4003 as terminal from the user's perspective.
3. **Drive admin UIs from `get_allowed_transitions_for`.** Render only the
   legal next states as buttons; do not hardcode the matrix client-side, and
   do not offer an "unblock" action to a non-admin caller.
4. **Pre-flight with `is_compliance_transition_allowed`** before building a
   transaction, but still handle 4006/4007 on submission — state can change in
   between.
5. **Index `compliance_status_changed`**, not the legacy events. It is the
   only event carrying the previous state, and it is emitted for every real
   transition regardless of which entrypoint caused it. See
   [`compliance-registry-reads.md`](compliance-registry-reads.md) for the
   projection and pagination strategy.

## Test coverage

Lifecycle tests live in [`src/test.rs`](../src/test.rs):

* **States & defaults** — `test_compliance_status_defaults_to_unknown`
* **Matrix** — `test_transition_matrix_matches_specification`,
  `test_self_transitions_are_never_allowed`,
  `test_unknown_is_never_a_transition_target`,
  `test_get_allowed_transitions_lists_exactly_the_legal_targets`,
  `test_get_allowed_transitions_for_address_tracks_current_state`
* **Valid & invalid transitions** — `test_full_happy_path_lifecycle_walk`,
  `test_invalid_transitions_are_rejected_and_leave_state_unchanged`,
  `test_unknown_to_revoked_is_rejected`,
  `test_every_invalid_transition_is_rejected_exhaustively` (all 25 pairs),
  `test_no_op_transition_reports_unchanged`
* **Authorization** — `test_compliance_officer_can_drive_the_lifecycle`,
  `test_emergency_officer_can_drive_the_lifecycle`,
  `test_only_admin_can_unblock`, `test_emergency_officer_cannot_unblock`,
  `test_unauthorized_caller_cannot_change_compliance_status`,
  `test_set_compliance_status_blocked_when_paused`
* **Mint/transfer enforcement** —
  `test_mint_rejects_each_non_approved_status_with_its_own_code`,
  `test_transfer_rejects_each_non_approved_sender_status`,
  `test_transfer_rejects_each_non_approved_receiver_status`,
  `test_sender_status_is_reported_before_receiver_status`,
  `test_blocking_a_holder_freezes_their_balance_without_destroying_it`
* **Events** — `test_set_compliance_status_emits_lifecycle_event`,
  `test_rejected_transition_emits_no_event`,
  `test_idempotent_whitelist_emits_no_duplicate_lifecycle_event`
* **Legacy compatibility** — `test_legacy_whitelist_wrappers_drive_the_lifecycle`,
  `test_legacy_whitelist_cannot_lift_a_block`,
  `test_legacy_revoke_does_not_downgrade_a_block`,
  `test_legacy_revoke_of_unknown_address_is_a_tolerated_no_op`
* **Eligibility & capabilities** —
  `test_eligibility_snapshot_exposes_lifecycle_status`,
  `test_check_transfer_eligibility_tracks_lifecycle_changes`,
  `test_capabilities_advertise_the_compliance_lifecycle`,
  `test_lifecycle_reads_never_revert_and_never_mutate`

## Pre-flight guards

Every precondition above — the pause, the caller's authority, the no-op rule,
and the matrix itself — is evaluated by one shared guard chain that clients can
read *before* submitting a transaction, and that the write path enforces. See
[`compliance-transition-guards.md`](compliance-transition-guards.md) for the
guard order, the typed refusal reasons, and the
`check_compliance_transition` / `check_compliance_batch` entrypoints.
