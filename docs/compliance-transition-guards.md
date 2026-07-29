# Compliance Status Transition Guards

This document specifies the **guards** that every compliance status change
must clear, the typed reason each refusal produces, and the pre-flight reads
that let an SDK or dashboard obtain that reason *before* an officer signs a
transaction.

> **Not legal or financial advice.** These guards are protocol-level access
> and state-machine controls only. Whether an investor may be approved,
> revoked, or frozen under a real-world regulatory regime is decided off-chain
> by the issuer's compliance and legal functions — see
> [`legal-boundary-disclaimer.md`](legal-boundary-disclaimer.md). The contract
> enforces that a recorded decision is *well-formed and authorized*; it does
> not make the decision.

Related: [`compliance-lifecycle.md`](compliance-lifecycle.md) defines the
states and the transition matrix; this document covers the guards layered on
top of it. [`compliance-status-transitions.md`](compliance-status-transitions.md)
covers the legacy two-transition model and its invariant tests.

## Why

The lifecycle already rejects illegal status changes. What it could not do was
*explain* a rejection ahead of time. A client had three bad options:

| Option | Problem |
| --- | --- |
| Re-implement the rules client-side | Two copies of a compliance-critical rule set, free to drift silently. The dashboard would confidently offer an action the contract refuses — or hide one it permits. |
| Submit and translate the revert | Burns a transaction, and a bare `Unauthorized` (3000) cannot distinguish "you need a role" from "this address is frozen and only the admin can act". |
| Read `is_compliance_transition_allowed` | Matrix-only. It ignores the caller, the pause, and the admin-only exit from `Blocked`, so `true` did not mean "this call will succeed". |

The guards close this by making one evaluation serve both purposes: the read
path and the write path call the *same function*. A pre-flight verdict cannot
disagree with enforcement, because there is nothing to disagree with.

## The guard chain

Every precondition is evaluated in a fixed order, and the **first** failure is
returned. The order matters and is itself a security property.

| # | Guard | Reason on failure | Error |
| --- | --- | --- | --- |
| 1 | Contract is initialized | `NotInitialized` | `NotInitialized` (2000) |
| 2 | Contract is not paused | `ContractPaused` | `ContractPaused` (3004) |
| 3 | Caller may act on the *current* status | `CallerUnauthorized` / `BlockedRequiresAdmin` | `Unauthorized` (3000) |
| 4 | Requested status differs from current | `StatusUnchanged` | `ComplianceStatusUnchanged` (4007) |
| 5 | Target is not `Unknown` | `TargetUnknownForbidden` | `InvalidComplianceTransition` (4006) |
| 6 | `from -> to` is in the transition matrix | `TransitionForbidden` | `InvalidComplianceTransition` (4006) |
| — | all pass | `Allowed` | — |
| batch only | Address appears once per batch | `DuplicateUserInBatch` | `InvalidComplianceTransition` (4006) |

Two ordering choices are deliberate:

- **Pause before authority.** A paused contract reports the pause to everyone.
  It never leaks whether the caller *would* have qualified, so the pre-flight
  read cannot be used to probe the role table during an incident.
- **Authority before the matrix.** An unauthorized caller learns nothing about
  which edges are legal for an address they may not touch.

### Authority: who may move which status

| Current status | ComplianceOfficer | EmergencyOfficer | Admin | Anyone else |
| --- | :---: | :---: | :---: | :---: |
| `Unknown` / `Pending` / `Approved` / `Revoked` | ✓ | ✓ | ✓ | ✗ |
| `Blocked` | ✗ | ✗ | ✓ | ✗ |

Leaving `Blocked` is admin-only, mirroring the pause/unpause asymmetry in
[`admin-roles.md`](admin-roles.md): a compromised or coerced compliance officer
must not be able to lift a sanctions freeze. *Entering* `Blocked` stays
available to any compliance role — freezing fast is the safe direction.

`BlockedRequiresAdmin` and `CallerUnauthorized` both map to `Unauthorized`
(3000) on-chain, but they are separate *reasons* because the remediation
differs: "escalate to the admin" versus "request a role". A client that
collapses them will tell a properly-credentialed officer to ask for a
permission they already hold.

## Reason codes

```rust
pub enum TransitionGuard {
    Allowed,
    NotInitialized,
    ContractPaused,
    CallerUnauthorized,
    BlockedRequiresAdmin,
    StatusUnchanged,
    TargetUnknownForbidden,
    TransitionForbidden,
    DuplicateUserInBatch,
}
```

Variants are **append-only**: never reorder or repurpose one. The variant order
is part of the contract ABI, under the same stability contract as the
[`error-codes.md`](error-codes.md) numeric codes and the
[`events.md`](events.md) topics.

`TargetUnknownForbidden` is separated from `TransitionForbidden` because
`Unknown` is unreachable from *every* source status — compliance history is
never erased, and offboarding is `Revoked`. A client should drop `Unknown` from
its target picker entirely rather than render an edge that can never succeed.

## Read entrypoints

All three are pure reads: no authorization, no writes, no events, and they
remain callable while the contract is paused.

### `check_compliance_transition(caller, user, new_status) -> ComplianceTransitionCheck`

```rust
pub struct ComplianceTransitionCheck {
    pub user: Address,
    pub caller: Address,
    pub current_status: ComplianceStatus,
    pub requested_status: ComplianceStatus,
    pub allowed: bool,
    pub reason: TransitionGuard,
    pub error_code: Option<u32>,
}
```

`current_status` is included so a client cannot race a separate
`get_compliance_status` read against this one and render an inconsistent pair.
`error_code` is pre-resolved to the numeric code a rejected submission would
revert with, so clients reuse their existing
[`error-codes.md`](error-codes.md) mapping instead of maintaining a second
reason table.

### `get_compliance_transition_guard(caller, user, new_status) -> TransitionGuard`

The reason alone, for clients that only branch on it.

### `check_compliance_batch(caller, updates) -> Vec<ComplianceTransitionCheck>`

One verdict per entry, in input order. Entries are evaluated independently —
which is sound precisely because duplicate addresses are rejected, so no entry
can change the status another is judged against.

**The batch is atomic.** A single `allowed == false` row means the whole
submission fails and *no* address is updated. Treat a rejected row as "this
batch will not commit", never as "this row will be skipped". This is the one
place where per-row `allowed == true` must not be read as "this row commits" —
the batch pre-flight tells you which row to fix, not what will partially apply.

## Enforcement path

`set_compliance_status`, `batch_set_compliance_status`, `whitelist_user`, and
`revoke_whitelist` all reach their verdict through the same evaluation. Two
details preserve backwards compatibility exactly:

- **Failure shape is unchanged.** Availability and authorization failures
  (guards 1–3) abort by panicking, as they always have; rule violations that
  are the caller's choice of edge (guards 4–6) return a typed `Err`. Existing
  SDK error handling, tests, and fixtures see identical behaviour.
- **Legacy wrappers stay tolerant.** `whitelist_user` and `revoke_whitelist`
  enforce only the *authority* half of the chain, keeping their documented
  idempotent no-op behaviour (re-approving an approved address succeeds;
  revoking an `Unknown` address is a no-op). They still cannot lift a freeze:
  `Blocked -> Approved` is refused, and `revoke_whitelist` never downgrades
  `Blocked` to the weaker `Revoked`.

## Security and compliance assumptions

These hold at the protocol level and are the assumptions a reviewer should
check against the issuer's off-chain controls:

1. **A guard verdict is point-in-time, not a reservation.** Nothing is locked
   between the read and the submission. A pause, a role revocation, or another
   officer's write can land in between, so every client must still handle a
   revert. Do not use `allowed == true` as an authorization decision on its own.
2. **The guard does not evaluate `require_auth`.** Whether the caller can
   produce a valid signature is a property of the submitted transaction, not of
   ledger state. `Allowed` means "the rules permit this caller", not "this
   caller is authenticated". A pre-flight read for an arbitrary `caller`
   address is therefore public information — treat the role table as public,
   because it is.
3. **The admin is trusted.** The admin bypasses every role check and is the
   sole authority able to lift `Blocked`. Admin-key compromise is out of scope
   for these guards; see [`admin-misuse-risks.md`](admin-misuse-risks.md) and
   [`threat-model.md`](threat-model.md).
4. **`Blocked` is a protocol freeze, not a sanctions determination.** The
   contract records that an enforcement decision was made off-chain and
   restricts who may reverse it. It performs no screening of its own.
5. **Reads leak status.** Compliance status and guard verdicts are readable by
   anyone with RPC access, as is all contract state. Do not store personal data
   on-chain; the lifecycle carries a status, never an identity.
6. **The pause is recoverable, never a lockout.** Guard 2 refuses everything
   while paused; the same edges clear after `unpause`. See
   [`emergency-pause.md`](emergency-pause.md).

## Test coverage

All tests are in [`src/test.rs`](../src/test.rs) under
`COMPLIANCE STATUS TRANSITION GUARDS`. The load-bearing ones assert
*agreement* rather than a hardcoded expectation, so they fail if the read path
and the write path ever diverge:

| Test | What it proves |
| --- | --- |
| `test_guard_matches_enforcement_for_every_edge_as_officer` | All 5 × 5 source/target edges: the pre-flight verdict and the real submission agree on outcome, error code, and resulting status. Each edge runs on a fresh deployment. |
| `test_guard_matches_enforcement_for_every_edge_as_admin` | The same 25 edges for the admin, covering the admin-only exit from `Blocked`. |
| `test_guard_matches_enforcement_for_every_edge_as_unauthorized_caller` | The same 25 edges for a wrong-scoped role: always refused, always predicted in advance. |
| `test_guard_reports_blocked_requires_admin_not_generic_unauthorized` | A credentialed officer is refused on a blocked address with the specific reason; the admin may move it only to `Pending`. |
| `test_guard_reports_status_unchanged_for_every_self_edge` | Every no-op is caught, for all five statuses. |
| `test_guard_reports_target_unknown_as_its_own_reason` | `Unknown` is unreachable from every source and reports its dedicated reason. |
| `test_guard_reports_pause_ahead_of_authority` | The pause is reported first for every caller class, reads stay callable while paused, and the edge clears after `unpause`. |
| `test_guard_reports_not_initialized_instead_of_panicking` | The read answers on an unconfigured deployment instead of reverting, and its prediction holds. |
| `test_guard_reads_never_mutate_state` | Repeated pre-flight reads change no status, role, or pause flag, and emit **no** events — a pre-flight is not a compliance action and must leave no audit trace. |
| `test_guard_verdict_tracks_role_revocation` | Authority is evaluated at call time, never cached: revoking an officer's role flips the verdict immediately, including for addresses they approved. |
| `test_guard_accepts_emergency_officer_and_rejects_asset_manager` | Role scoping is exact. |
| `test_batch_guard_matches_batch_execution_when_every_entry_is_legal` | A fully-legal batch pre-flights clean and commits. |
| `test_batch_guard_flags_the_offending_entry_and_the_batch_fails_atomically` | The guard pinpoints the offending row; the batch fails whole and the legal row is not applied. |
| `test_batch_guard_flags_duplicate_addresses` | Only the repeat is flagged, and the batch is rejected. |
| `test_batch_guard_accepts_an_empty_batch` | Empty batches are legal and commit nothing. |
| `test_guard_agrees_with_the_legacy_whitelist_entrypoints` | The guard predicts the authorization outcome of `whitelist_user` across all five source statuses, including where the wrapper's idempotence absorbs a no-op. |

Run them with `make test`.

## Client guidance

- **Render the reason, not the boolean.** `allowed == false` with
  `BlockedRequiresAdmin` is an escalation, not an error message.
- **Re-check after any state change.** Verdicts are not cacheable across
  ledgers. Pause state and role assignments both invalidate them.
- **Use `get_allowed_transitions_for(user)` to build the picker, then
  `check_compliance_transition` to confirm the caller may take it.** The first
  answers "what edges exist", the second "may *this* officer take one now".
- **Never treat a verdict as a substitute for handling the revert.** See
  assumption 1.

## Maintenance

Any new compliance write entrypoint **must** reach its verdict through
`compliance_guards::require_transition` (or `require_transition_authority` for
a tolerant legacy-style wrapper). Adding a precondition means adding a
`TransitionGuard` variant, its error mapping, a row in the guard-chain table
above, and a case in the agreement tests — in the same change. A precondition
enforced outside the guard is a silent divergence between what clients are told
and what the contract does, which is exactly the failure mode this module
exists to prevent.
