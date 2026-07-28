# Aegis Contracts — Contributor Evaluation Policy

> **This is the formal evaluation policy for all Aegis RWA Contracts
> contributions. All contributors are expected to read and follow it.
> Violations may affect current and future reward eligibility.**

## Scope

This policy applies to every Pull Request submitted to the Aegis Contracts
repository, regardless of whether the contribution is part of a paid campaign
(e.g. GrantFox) or an unpaid open-source contribution. It defines the
standards, process, and expectations for contribution evaluation.

---

## 1. Merge Does Not Equal Payment

A merged Pull Request is a technical acceptance of the code — it means the
change compiles, passes tests, and is acceptable for the codebase. **It is not
a guarantee of payment or reward approval.**

- Payment decisions are made separately through the GrantFox evaluation process
  (see [Section 4](#4-grantfox-evaluation-process)).
- A `Maybe Rewarded` label or any campaign tag is discretionary — it signals
  the issue *may* qualify, not that it *will* be rewarded.
- Reward eligibility depends on issue tier, per-campaign paid-issue caps, and
  available campaign budget. None of these are decided by the contributor.

**Stated plainly: merge and payment are separate events.**

---

## 2. Self-Review Is Required Before Evaluation

Before requesting review or expecting payment consideration, every contributor
must complete a thorough self-review. This is not optional.

### Minimum self-review checklist:

- [ ] Every acceptance criterion from the issue is satisfied and mapped in the
      PR description via the [Traceability Mapping](./traceability-mapping.md)
      completion table.
- [ ] `make build` and `make test` pass locally (see
      [Local Verification](./local-verification.md)).
- [ ] Code is minimal, readable, and free of `#[allow]`-hidden warnings.
- [ ] Documentation and README are updated if the change affects them.
- [ ] PR description is clear about what changed and why.

**If you would not merge it yourself, do not submit it for evaluation.**

📖 **Full guide:** [Contributor Self-Review Form](./contributor-self-review-form.md)

---

## 3. Maintainer Review Standards

Maintainers evaluate contributions against the following criteria:

| Criterion | Standard |
| :--- | :--- |
| **Correctness** | Implementation matches the documented spec and compliance model |
| **Security** | Authorization, bounds, invariants, and pause controls are enforced |
| **Test coverage** | Happy-path and negative-path tests exist and pass |
| **CI status** | All automation checks are green |
| **Acceptance criteria** | Every issue criterion is met or explicitly documented as deferred |
| **Documentation** | Relevant docs, events, error codes, and README are updated |
| **Self-review** | The [Self-Review Form](./contributor-self-review-form.md) is completed |

A PR that fails any of these criteria will be returned for revision before
evaluation proceeds.

📖 **Full guide:** [Reviewer Checklist](./reviewer-checklist.md)

---

## 4. GrantFox Evaluation Process

Rewards for paid-campaign issues follow the GrantFox flow:

1. **Maintainer assignment** — a maintainer assesses whether the completed work
   meets the issue's bar against the standards in [Section 3](#3-maintainer-review-standards).
2. **Program approval** — the GrantFox admin team approves against the
   campaign's limited budget and per-contributor paid-issue caps.
3. **Payout** — released only after the campaign ends, via Stellar USDC escrow.

Because approval happens in steps and against a finite budget, **a merged PR
alone does not trigger payment**. Contributors should expect a delay between
merge and any reward decision.

---

## 5. Testing and CI Expectations

Every contribution must meet the repository's testing standards:

- All contract changes must build for `wasm32v1-none` and pass the full test
  suite — locally *and* in CI.
- Run `make verify` (fmt-check + clippy + test + build) before pushing.
- A failing CI check is a blocker — fix it before requesting review. See the
  [Failing CI Response Guide](./failing-ci-guide.md).
- Every state-changing function must have happy-path and negative-path tests.
- Tests must assert events, error codes, and state invariants.
- SDK integration fixtures must be verified or updated if contract output
  changes.

**Quality matters more than raw PR count.** Submitting low-effort or
incomplete work to chase payment volume is discouraged and may affect future
campaign eligibility.

📖 **Full guide:** [Minimum Testing Standards](./testing-standards.md)

---

## 6. Acceptance Criteria Completion

Every PR must demonstrate that all acceptance criteria from the linked issue
are satisfied:

1. Include a [Completion Table](./traceability-mapping.md#completion-table-format)
   in the PR description mapping every criterion to its status, implementation
   evidence, and test evidence.
2. Status must be one of: **Complete**, **Partial** (with explanation), or
   **Not Met** (with rationale).
3. Incomplete criteria must include a follow-up plan or link to a tracking
   issue.
4. For complex PRs, include the
   [Detailed Traceability Table](./traceability-mapping.md#detailed-traceability-table-format-advanced).

A PR with incomplete acceptance criteria and no justification will be returned
without evaluation.

📖 **Full guide:** [Requirement Traceability Mapping](./traceability-mapping.md)

---

## 7. Conduct During Payment Periods

Contributors working during paid campaigns must follow these conduct rules:

- **Do not spam community channels.** Repeated complaints about "when will I
  be paid" or "why isn't my issue rewarded" flood channels and slow everyone
  down. Ask once, clearly, with a link to your PR — then wait for a maintainer
  or program response.
- **Evaluation queues have many contributors.** Silence means "in review,"
  not "ignored." Do not ping repeatedly.
- **Keep all discussion technical and respectful.** Toxic, hostile, or spammy
  behavior is a direct violation of this policy.
- **Disagreements about reward decisions** belong in the evaluation thread,
  not in general chat. Taking disputes into public channels is a conduct
  violation.

Violations may result in suspension from current and future paid campaigns,
removal from community channels, or permanent disqualification from reward
consideration.

📖 **Full guide:** [Payment-Period Conduct Note](./payment-period-conduct.md)

---

## 8. Policy Violations

Violations of this policy — including but not limited to spam, harassment,
payment-rigging attempts, submitting knowingly incomplete work for reward,
or circumventing evaluation — will be addressed proportionally:

| Violation | Consequence |
| :--- | :--- |
| First minor infraction | Written warning |
| Repeated or serious infraction | Suspension from current campaign |
| Severe or malicious violation | Permanent disqualification from all future campaigns |

---

## Related Documents

- [Evaluation Readiness Summary](./evaluation-readiness.md) — central checklist for contributors before evaluation day
- [PR Evidence Checklist](./pr-evidence-checklist.md) — mandatory evidence checklist for every PR
- [Meaningful Implementation Checklist](./meaningful-implementation-checklist.md) — what counts as real contract work
- [Aegis Contracts Contribution Examples](./aegis-contracts-examples.md) — side-by-side comparisons of acceptable vs unacceptable contributions
- [CONTRIBUTING.md](../CONTRIBUTING.md) — development workflow and branching conventions
