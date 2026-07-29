# Reviewer Evidence Checklist

This checklist helps Aegis Contracts maintainers decide whether a pull request
has enough evidence to be considered complete and evaluation-ready. It is a
review aid, not a replacement for the contributor-facing
[PR Evidence Checklist](./pr-evidence-checklist.md) or the broader
[Reviewer Checklist](./reviewer-checklist.md).

Use it before approving, merging, or marking a contribution ready for campaign
evaluation.

## 1. Scope Evidence

- [ ] The PR links to the issue it claims to resolve.
- [ ] The diff is limited to the issue scope and does not include unrelated
      refactors, generated files, dependency churn, or formatting-only changes.
- [ ] Any out-of-scope work is explicitly removed or tracked in a separate
      follow-up issue.
- [ ] The PR description explains why each touched file is necessary.

## 2. Implementation Evidence

- [ ] The implementation satisfies the behavior described in the issue, not
      only the title.
- [ ] Contract changes preserve existing authorization, pause, compliance,
      supply-cap, and transfer-restriction invariants.
- [ ] New public functions, events, errors, storage keys, or role checks are
      documented in the PR description and relevant docs.
- [ ] The change is readable, minimal, and does not hide warnings with
      broad `#[allow]` attributes.

## 3. Test Evidence

- [ ] Happy-path behavior is covered by focused tests.
- [ ] Negative paths cover unauthorized callers, invalid state transitions,
      boundary values, and expected contract errors where applicable.
- [ ] Existing tests still pass, and fixture-impacting changes either preserve
      or intentionally update SDK fixture expectations.
- [ ] If tests were not added, the PR includes a clear no-test justification
      that is acceptable under [Minimum Testing Standards](./testing-standards.md).

## 4. CI and Local Verification Evidence

- [ ] The contributor reports the exact commands run, including `make verify`
      or the relevant subset such as `make fmt-check`, `make clippy`,
      `make test`, and `make build`.
- [ ] GitHub Actions are green before approval, or any failing job has a
      documented, issue-linked reason that is not caused by the PR.
- [ ] Local failures, if any, are reproduced and explained rather than ignored.
- [ ] The reviewer can reproduce the key verification commands when the change
      touches contract behavior or fixtures.

## 5. Documentation Evidence

- [ ] README links, contributor docs, contract docs, and SDK/dashboard guidance
      are updated when the behavior affects users or downstream consumers.
- [ ] Documentation avoids regulatory or legal promises beyond the contract's
      on-chain enforcement model.
- [ ] New docs include enough context for a future maintainer to verify the
      change without reading the full issue thread.
- [ ] Existing docs are not contradicted by the new change.

## 6. Acceptance-Criteria Evidence

- [ ] Every acceptance criterion from the issue is mapped to implementation
      evidence, test evidence, and status.
- [ ] Partial or deferred criteria are explicitly marked and justified.
- [ ] The reviewer checks the diff against the acceptance criteria directly
      rather than relying only on the contributor summary.
- [ ] A PR with missing criteria is returned for revision before evaluation.

## 7. Risk and Evaluation Notes

- [ ] Security-sensitive changes receive extra review for authorization,
      storage, event, and invariant impact.
- [ ] The reviewer records any known limitations that remain after merge.
- [ ] Merge readiness and reward evaluation are treated as separate decisions.
- [ ] If the PR is part of a paid campaign, the reviewer confirms that the
      evidence is sufficient for evaluation but does not imply payout approval.

## Reviewer Sign-off

Before approving, leave a short note covering:

1. the issue and acceptance criteria reviewed;
2. the commands or CI results used as evidence;
3. any docs or fixture impact checked;
4. any limitations, waivers, or follow-up issues.

If any section above is not satisfied, request changes instead of approving the
pull request for evaluation.
