# Compliance status transitions

> This document describes the **legacy two-transition model**
> (`whitelist_user` / `revoke_whitelist`) and its invariant tests. The current
> five-state model is specified in
> [`compliance-lifecycle.md`](compliance-lifecycle.md), and the guards that
> gate every status change — with the typed refusal reasons and the pre-flight
> reads that share them — in
> [`compliance-transition-guards.md`](compliance-transition-guards.md).

This document defines the compliance status state machine for investor
addresses — the **approved**, **revoked**, **blocked**, **pending**, and
**unknown** statuses — the transitions that are valid under authorised and
unauthorised callers, and the **invariant tests** that lock the model in for
audit readiness. It is the reference for the test section
`COMPLIANCE STATUS TRANSITION INVARIANTS` in [`src/test.rs`](../src/test.rs).

## Why

Invalid compliance transitions create two classic bugs in RWA tokenization:

- **Bypass bugs** — an address that should not be tradable becomes tradable
  (e.g. a failed or unauthorised approval accidentally whitelists it, or a
  revoked investor keeps moving assets).
- **Lockout bugs** — an address that should be recoverable becomes stuck
  (e.g. a blocked transition during a pause permanently wedges the registry,
  or revoking one officer's role bricks every address they managed).

The contract enforces the model below at execution time; the invariant tests
prove the guards hold deterministically across the entire status × action ×
caller space and that rejected transitions are fully atomic.

## Status model

The compliance registry is an address-keyed set
([`compliance-registry-reads.md`](compliance-registry-reads.md)), so an
investor's compliance status is derived from observable contract state plus
the transition history of the address:

| Status    | Definition                                                | Observable signal            |
|-----------|-----------------------------------------------------------|------------------------------|
| `Unknown` | Never targeted by any compliance call.                    | `is_whitelisted` = `false`   |
| `Pending` | An approval attempt was rejected (wrong caller or paused contract); the address is still awaiting a valid approval. | `is_whitelisted` = `false` |
| `Approved`| A `whitelist_user` call committed for the address.        | `is_whitelisted` = `true`    |
| `Revoked` | A `revoke_whitelist` call committed after `Approved`.     | `is_whitelisted` = `false`   |
| `Blocked` | **Global overlay, not a per-address status**: while the contract is paused, *every* compliance transition attempt reverts with `ContractPaused` (`3004`), regardless of address status or caller. | `is_paused` = `true` |

`Unknown`, `Pending`, and `Revoked` are intentionally indistinguishable
on-chain (all read as "not whitelisted"); they differ only in how the address
got there, which is exactly what an event indexer reconstructs from the
committed `user_whitelisted` / `whitelist_revoked` events.

## Transitions

The contract exposes exactly two compliance transitions:

| Action (function) | Resulting status on commit | Idempotence |
|---|---|---|
| `whitelist_user(caller, user)` — **Approve** | `Approved` | Re-approving an already `Approved` address is an idempotent success that re-emits `user_whitelisted`. |
| `revoke_whitelist(caller, user)` — **Revoke** | off the whitelist (`Revoked`) | Revoking a non-approved address (`Unknown`, `Pending`, already `Revoked`) is an idempotent no-op that still emits `whitelist_revoked` for audit-indexer simplicity. |

### Authorised callers

`Admin` (bypasses role checks), `ComplianceOfficer`, and `EmergencyOfficer`
may commit both transitions:

| From \ Action | Approve | Revoke |
|---|---|---|
| `Unknown`  | → `Approved` | → stays off whitelist (no-op commit) |
| `Pending`  | → `Approved` | → stays off whitelist (no-op commit) |
| `Approved` | → `Approved` (idempotent) | → `Revoked` |
| `Revoked`  | → `Approved` (re-onboarding is allowed; revocation is not a permanent lockout) | → stays `Revoked` (no-op commit) |

### Unauthorised callers

Every transition attempted by an address with **no role**, or with a
wrong-scoped role (e.g. `AssetManager`), is **rejected** with
`Unauthorized` (`3000`), the address status is left exactly as it was, and no
event is emitted. This includes callers whose compliance role was revoked
after they performed earlier approvals — role checks are evaluated at call
time, not cached.

### Blocked (paused) overlay

While the contract is paused, the pause guard runs **before** role checks, so
**every** transition attempt — from any status, with any action, by any
caller, authorised or not — is rejected with `ContractPaused` (`3004`), emits
no event, and changes nothing. After `unpause`, authorised transitions work
again: the blocked overlay never becomes a permanent lockout.

## Invariants asserted by the test suite

All tests live in [`src/test.rs`](../src/test.rs) under
`COMPLIANCE STATUS TRANSITION INVARIANTS` and run against a fresh contract
deployment per matrix row, so results are deterministic and independent of
test execution order.

| Test | What it proves |
|---|---|
| `test_compliance_transition_matrix_deterministic` | The full 4 statuses × 2 actions × 5 caller classes (40 rows): authorised transitions always commit to the exact target status; unauthorised transitions are always `Unauthorized` and never change status. |
| `test_compliance_transitions_blocked_when_paused` | The blocked overlay: 40 (status × action × caller) rows while paused all revert with `ContractPaused`, state unchanged, zero events; the same rows resolve correctly after unpause (no lockout). |
| `test_compliance_transition_events_have_exact_shape` | Committed transitions emit exactly one event with the exact topic and payload (`user_whitelisted` / `whitelist_revoked`), and the `caller` field records the actual role caller, not just the admin. |
| `test_rejected_compliance_transitions_emit_no_events` | Unauthorised and blocked attempts emit no events (Soroban discards events from reverted invocations). |
| `test_failed_compliance_transitions_leave_state_consistent` | Rejected transitions are atomic: target status, bystander status, balances, total supply, roles, asset status, and pause flag are all unchanged afterwards; valid transitions still succeed after the failures (registry not wedged); revocation freezes but never destroys a balance. |
| `test_compliance_status_lifecycle_invariants` | The state-machine walk `Unknown → Pending → Approved → Revoked → Re-approved`: non-approved statuses can never receive a mint (`ReceiverNotWhitelisted`), a revoked investor can neither receive nor send (`SenderNotWhitelisted`), and re-approval restores full rights — the direct bypass/lockout regression guards. |
| `test_compliance_transitions_rejected_after_officer_role_revoked` | Wrong-caller nuance: after the admin strips a compliance officer's role, that officer's further transitions are `Unauthorized` with no state change and no event — even for addresses they personally approved — while the admin can still act. |

Related coverage elsewhere in the suite: `test_invalid_input_matrix_full_coverage`
(invalid-input matrix), the `wrong-caller: whitelist_user / revoke_whitelist`
tests, the `Pause: blocked state-changing operations` tests, and the event
compatibility tests asserted against [`events.md`](events.md).

## Event guarantees

- A **committed** transition emits exactly one event per invocation —
  `user_whitelisted(caller, user)` or `whitelist_revoked(caller, user)`
  (see [`events.md`](events.md)).
- **Idempotent** transitions (re-approve, revoke-of-non-approved) also emit
  their event, so an indexer sees every committed call in order.
- A **rejected** transition (unauthorised, paused, or any other revert)
  emits **no** event — Soroban discards events from reverted invocations.
  The standardized error codes (`3000`, `3004`) are the off-chain-observable
  signals for rejected attempts (see [`error-codes.md`](error-codes.md)).

## State consistency guarantees

- Rejected transitions are atomic: no whitelist flag, balance, supply, role,
  lifecycle status, or pause state changes on any failure path.
- Failures are isolated per address: a rejected transition targeting one
  investor never affects a bystander's compliance status or balance.
- Revocation freezes an investor (no mint-in, no transfer-out) but never
  destroys their balance; re-approval restores full movement rights.
- Neither a blizzard of failed attempts nor a pause/unpause cycle can wedge
  the registry: the first valid call afterwards behaves exactly as specified
  in the transition matrix.

## Maintenance

Any new compliance entry point (e.g. batch whitelisting, investor tiers —
both tracked in [`capabilities.md`](capabilities.md)) **must** extend the
deterministic matrix (`ComplianceStatus`, `ComplianceAction`,
`TransitionCaller`) and the tables in this document in the same change.
