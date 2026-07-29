# Issuer Role Separation

This document specifies the separation-of-duties controls the Aegis RWA
Contracts apply to **issuance**: which duties each role actually carries, how a
deployment can require that clearing an investor and funding that investor be
two different keys, and what the controls do and do not defend against.

> **Not legal or financial advice.** These are protocol-level access controls.
> Whether an issuer's operating model satisfies a real-world regulatory
> requirement for segregation of duties is determined off-chain by that
> issuer's compliance, audit, and legal functions — see
> [`legal-boundary-disclaimer.md`](legal-boundary-disclaimer.md). The contract
> can enforce that two distinct keys acted; it cannot attest that two distinct
> *people* did.

## Why

The role model in [`admin-roles.md`](admin-roles.md) answers *which privileges
an address holds*. It does not answer the question an RWA auditor asks first:

> Can the same key both decide **who may hold** the asset and decide **who
> receives** it?

Until now the answer was yes. The supreme admin bypasses every role check, so a
single admin key can approve an address and then mint to it, with no second
party involved and nothing in the contract to notice. That is the classic
control failure behind fictitious-holder and self-allocation issuance fraud:
not a bug in any one function, but the absence of a constraint *between* two
correctly-functioning ones.

This module adds that constraint, as an **opt-in policy** so no existing
deployment's behaviour changes until an admin enables it.

## Duties, not role names

Roles are the unit of *assignment*; duties are the unit of *separation*. The
duty map is derived from what the contract **enforces**, not from what a role
is named after:

| Role | Compliance | Issuance | Emergency | Governance |
| --- | :---: | :---: | :---: | :---: |
| `None` | | | | |
| `ComplianceOfficer` | ✓ | | | |
| `AssetManager` | | ✓ | | |
| `EmergencyOfficer` | ✓ | | ✓ | |
| `Admin` | ✓ | ✓ | ✓ | ✓ |

> **Correction to an earlier claim.** `admin-roles.md` previously listed
> `mint_asset` and `distribute_yield` among `EmergencyOfficer`'s privileged
> operations. That has never been true in the code: both entrypoints call
> `require_role(AssetManager)`, which admits an `AssetManager` or the admin and
> nobody else. An `EmergencyOfficer` minting is rejected with `Unauthorized`
> (3000). The duty table above and `admin-roles.md` now match the contract.
> A separation control built on an inaccurate privilege map is worse than
> none, which is why this is stated rather than quietly fixed.

The consequence: **today the admin is the only address carrying both the
compliance and issuance duties.** The dual-duty control below is therefore, in
practice, the control that forces an admin to delegate issuance to a dedicated
`AssetManager` key. It is written against duties rather than against "the
admin" so that it keeps working if a future role combines them.

The map is readable on-chain: `get_role_duties(role)` and
`get_duties_of(address)` (which resolves the supreme admin to `Role::Admin`
whatever the role table says).

## The policy

```rust
pub struct IssuerSeparationPolicy {
    pub enforced: bool,                       // master switch
    pub allow_dual_duty_issuance: bool,
    pub allow_self_issuance: bool,
    pub require_independent_approver: bool,
}
```

| Control | When it refuses | Error |
| --- | --- | --- |
| `allow_dual_duty_issuance: false` | The caller carries **both** the compliance and issuance duties. | `IssuanceDutyConflict` (3007) |
| `allow_self_issuance: false` | The caller is the recipient. | `SelfIssuanceForbidden` (3008) |
| `require_independent_approver: true` | The caller is the recorded approver of the recipient's compliance. | `IssuanceApproverConflict` (3009) |

The default, applied when no policy has been stored, is **fully permissive**:
`enforced: false` with every control relaxed. Adding this module changes no
deployment's behaviour; enabling separation is a deliberate, audited act.

Controls are independent. An issuer can adopt only the parts their operating
model supports — for example allowing a dual-duty admin to issue generally
while still forbidding it from funding investors it personally cleared
(`allow_dual_duty_issuance: true, require_independent_approver: true`).

### Evaluation order

The first failing control is returned, in this order:

1. contract initialized → `NotInitialized`
2. caller carries the issuance duty → `MissingIssuanceDuty` (an RBAC failure,
   **not** a separation failure — `IssuanceGuard::is_separation_failure()`
   distinguishes them)
3. `enforced`? if not, allow
4. dual duty → `DualDutyConflict`
5. self-issuance → `SelfIssuanceForbidden`
6. approver identity → `ApproverConflict`

`distribute_yield` has no single beneficiary, so steps 5 and 6 do not apply to
it; only the duty-level control binds.

## The approver record

`DataKey::ComplianceApprover(Address)` records the caller of the most recent
committed transition **into** `ComplianceStatus::Approved` — via
`set_compliance_status`, `batch_set_compliance_status`, or the legacy
`whitelist_user`. It is exposed as `get_compliance_approver(user)`.

Two deliberate choices:

- **A revocation does not erase it.** Revoking clearance should not erase who
  granted the clearance being revoked. Re-approval by a different officer
  overwrites it.
- **Only the most recent approver is kept.** This is a control against one key
  performing both steps, not a full historical audit. Reconstruct the complete
  history from `compliance_status_changed` events
  ([`events.md`](events.md)) — the contract deliberately stores one address,
  not an unbounded list.

## Governance

`set_issuer_separation_policy(admin, policy)` — admin-only, blocked while
paused, emits `issuer_separation_policy_updated` carrying **both** the previous
and the new policy so an auditor can reconstruct when each control was in force
without replaying storage.

Applied in a single call rather than through the 2-step flow used for cap
amendments: unlike a cap, tightening separation cannot strand value, and an
issuer responding to a suspected key compromise should not have to wait for a
second transaction to close the gap.

**The policy can never lock a deployment out of issuance.** The setter is
deliberately not gated by the policy it sets, so if the strictest configuration
leaves no key able to mint, the admin relaxes it and issuance resumes. This is
verified by `test_separation_policy_can_never_lock_a_deployment_out_of_issuance`.

## Pre-flight read

`check_issuance_authority(caller, recipient) -> IssuanceAuthorityCheck` returns
the verdict from the **same evaluation `mint_asset` enforces**, so `allowed ==
false` guarantees a mint reverts with `error_code`. It reports the caller's
effective role and duties and the recipient's recorded approver, so a dashboard
can explain a refusal rather than just reporting one.

```rust
pub struct IssuanceAuthorityCheck {
    pub caller: Address,
    pub recipient: Address,
    pub caller_role: Role,
    pub caller_duties: Vec<IssuerDuty>,
    pub recipient_approver: Option<Address>,
    pub allowed: bool,
    pub reason: IssuanceGuard,
    pub error_code: Option<u32>,
}
```

**Separation only.** This answers "may this key issue to this address", not
"will this mint succeed". The recipient's compliance status, the supply and
holding caps, the asset lifecycle, the pause, and the amount are checked
separately — use `check_mint_restriction`
([`transfer-restrictions.md`](transfer-restrictions.md)) for those.

## Security and compliance assumptions

1. **This is not a defence against a compromised admin key.** An attacker
   holding the admin key can lift the policy in one transaction and then issue.
   These controls raise the cost of routine key misuse and operator error, and
   they make both steps visible on-chain — they do not constrain an adversary
   who already controls governance. See
   [`admin-misuse-risks.md`](admin-misuse-risks.md) and
   [`threat-model.md`](threat-model.md).
2. **Two keys are not two people.** The contract can only observe that distinct
   addresses acted. Whether they are controlled by different individuals under
   different approval chains is an off-chain organizational control that this
   module assumes, and cannot verify.
3. **Only the most recent approver is enforced against.** An officer who
   approves an investor, has a colleague re-approve them, and then issues will
   pass the approver control. Four-eyes on the *approval* itself is off-chain.
4. **Duties are derived, not stored.** They come from the role table plus
   `DataKey::Admin` at call time, so a role change takes effect immediately and
   nothing is cached that could go stale.
5. **A verdict is point-in-time.** A policy change, a role change, or a
   re-approval can land between a pre-flight read and a submission. Clients
   must still handle a revert.
6. **The policy is a protocol control, not an attestation.** It records that
   the issuer configured a separation requirement. It makes no claim about the
   issuer's off-chain governance.

## Test coverage

All tests are in [`src/test.rs`](../src/test.rs) under
`ISSUER ROLE SEPARATION`.

| Test | What it proves |
| --- | --- |
| `test_issuer_separation_is_off_by_default` | The default policy is permissive and every caller that could mint before still can — adding the module changes nothing. |
| `test_role_duty_table_is_exact` | The duty map matches enforced privileges for all five roles, and the admin resolves to the full duty set by address. |
| `test_dual_duty_issuance_is_refused_when_separation_is_enforced` | The dual-duty control binds the admin, leaves its other privileges intact, and leaves a scoped `AssetManager` unaffected. |
| `test_self_issuance_is_refused_when_disallowed` | An issuer cannot mint to itself; issuing to anyone else is untouched. |
| `test_independent_approver_control_enforces_four_eyes` | The approver of a recipient cannot fund that recipient; a different issuer can, and the same caller can fund investors someone else cleared. |
| `test_approver_record_tracks_every_approval_path` | The record is written by `set_compliance_status`, `whitelist_user`, and batch updates; survives revocation; is overwritten on re-approval; and is never created by a non-approving transition. |
| `test_separation_controls_are_independent` | Each control refuses on its own, so no test passes because a different rule fired. |
| `test_missing_issuance_duty_is_reported_separately_from_a_separation_failure` | An RBAC failure and a separation failure are distinguishable by the client. |
| `test_check_issuance_authority_matches_mint_enforcement` | Across five policies × four caller classes × two recipients, the pre-flight verdict and the real mint agree on outcome, error code, and resulting balance. |
| `test_issuance_check_reports_a_consistent_snapshot` | The report's role, duties, and approver fields are internally consistent. |
| `test_issuance_reads_never_mutate_state` | Reads change no balance, supply, role, or policy, and emit no events. |
| `test_policy_update_is_admin_only_and_emits_the_previous_policy` | Non-admins are refused; the event carries both policies with the exact shape. |
| `test_policy_update_is_blocked_while_paused` | Governance respects the global pause, and the read stays available during it. |
| `test_separation_policy_can_never_lock_a_deployment_out_of_issuance` | The strictest policy is always recoverable by the admin. |
| `test_yield_distribution_respects_the_duty_control_only` | Recipient-scoped controls cannot apply to a call with no beneficiary. |
| `test_issuance_guard_reports_not_initialized_instead_of_panicking` | The reads answer on an unconfigured deployment instead of reverting. |

Run them with `make test`.

## Recommended operating model

For an issuer adopting these controls from scratch:

1. Assign a dedicated `AssetManager` key that holds **no** compliance role.
2. Assign one or more `ComplianceOfficer` keys.
3. Keep the admin key in cold storage for governance only.
4. Set `enforced: true, allow_dual_duty_issuance: false, allow_self_issuance:
   false, require_independent_approver: true`.
5. Verify with `check_issuance_authority` that the admin key is refused and the
   `AssetManager` key is permitted — the intended shape of the separation.

Reviewers should confirm steps 1–5 against
[`reviewer-checklist.md`](reviewer-checklist.md) before a production deployment.

## Maintenance

Any new issuance entrypoint **must** call
`issuer::require_issuance_authority`, passing the recipient (or `None` when
there is no single beneficiary). Adding a control means adding an
`IssuanceGuard` variant, its error mapping, a policy field defaulting to the
permissive value, a row in the tables above, and a case in
`test_check_issuance_authority_matches_mint_enforcement` — in the same change.
`IssuerDuty`, `IssuanceGuard`, and `IssuerSeparationPolicy` are append-only
ABI: never reorder or repurpose a variant or field.
