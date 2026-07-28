# Compliance Batch Updates

This document specifies the batch compliance lifecycle operation for Aegis RWA
Contracts.

> **Not legal or financial advice.** This operation records protocol-level
> compliance status outcomes that have already been determined off-chain by
> the issuer's compliance and legal functions. It does not decide investor
> eligibility, sanctions status, jurisdiction, accreditation, or suitability.

## API

```rust
pub struct ComplianceBatchUpdate {
    pub user: Address,
    pub new_status: ComplianceStatus,
}

pub fn batch_set_compliance_status(
    caller: Address,
    updates: Vec<ComplianceBatchUpdate>,
) -> Result<u32, Error>
```

`batch_set_compliance_status` applies many compliance lifecycle transitions in
one invocation. It returns the number of applied updates.

The operation uses the same lifecycle rules as
[`set_compliance_status`](compliance-lifecycle.md):

- `Approved` is still the only status that can send or receive assets.
- `Unknown` is never a target.
- Self-transitions are rejected with `ComplianceStatusUnchanged` (4007).
- Illegal lifecycle moves are rejected with `InvalidComplianceTransition`
  (4006).
- Moving an address out of `Blocked` still requires the supreme admin.

## Atomicity

The batch is all-or-nothing. The contract validates every entry before writing
any status. If one entry fails, no address in the batch is updated and no
`compliance_status_changed` event is emitted.

This is deliberate for compliance-sensitive workflows: downstream systems
should never have to reconcile a partially-applied onboarding or remediation
file.

## Edge Cases

| Case | Result |
| --- | --- |
| Empty `updates` vector | Succeeds and returns `0`; emits no events. |
| Duplicate `user` in one batch | Reverts with `InvalidComplianceTransition` (4006). |
| Any no-op transition | Reverts with `ComplianceStatusUnchanged` (4007); no writes. |
| Any invalid transition | Reverts with `InvalidComplianceTransition` (4006); no writes. |
| Contract paused | Reverts with `ContractPaused` (3004); no writes. |
| Caller lacks compliance authority | Reverts with `Unauthorized` (3000); no writes. |
| Non-admin tries to move a `Blocked` address | Reverts with `Unauthorized` (3000); no writes. |

Duplicate entries are rejected rather than interpreted sequentially. A batch is
a set of intended final status changes, not a script; rejecting duplicates keeps
operator intent auditable and prevents order-dependent outcomes.

## Events

On success, the contract emits one `compliance_status_changed` event per
updated address, in the same order as the input vector. There is no separate
batch summary event; indexers can use the transaction envelope plus ordered
per-address events.

Rejected batches emit no events because Soroban discards events from reverted
invocations.

## SDK and Dashboard Guidance

- Build each row as a typed `ComplianceBatchUpdate`.
- Pre-flight rows with `get_compliance_status`, `get_allowed_transitions_for`,
  and `is_compliance_transition_allowed`, but still handle submission errors
  because state can change between simulation and submission.
- De-duplicate addresses before building the transaction.
- Treat a batch failure as a whole-file rejection. Re-read all affected
  addresses before retrying.
- Do not include KYC documents, legal notes, personal data, or sanctions
  evidence on-chain. Store only the resulting status.

## Tests

Coverage lives in [`src/test.rs`](../src/test.rs):

- `test_batch_set_compliance_status_updates_many_addresses_atomically`
- `test_batch_set_compliance_status_rejects_invalid_entry_without_partial_write`
- `test_batch_set_compliance_status_rejects_duplicates_and_no_ops`
- `test_batch_set_compliance_status_handles_empty_and_paused_batches`
