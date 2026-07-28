## Description
<!-- Describe your changes in detail here -->

## Related Issues
<!-- Link to any related issues (e.g., Fixes #123) -->

## Completion Table

<!-- REQUIRED: Map every acceptance criterion from the issue using the table below.
     See docs/traceability-mapping.md#completion-table-format for details.
     Status: Complete / Partial / Not Met -->

| Acceptance Criterion | Status | Implementation Evidence | Test Evidence |
| :--- | :---: | :--- | :--- |
| **AC 1**: | Complete / Partial / Not Met | | |
| **AC 2**: | Complete / Partial / Not Met | | |

> **Incomplete criteria must include a reason and follow-up plan.** See Handling Incomplete Criteria in docs/traceability-mapping.md.

## Detailed Traceability Mapping

<!-- For complex PRs involving storage changes, events, or security controls,
     include the detailed table below. See docs/traceability-mapping.md for
     examples. -->

| Acceptance Criteria | Implementation | Storage & State Changes | Events Emitted | Test Coverage | Security/Safety Controls |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **AC 1**: | | | | | |
| **AC 2**: | | | | | |

## Type of Change
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update
- [ ] Chore (refactoring, build tools, etc.)

## PR Evidence Checklist

Before opening this PR, complete every applicable item in the evidence
checklist below. See [PR Evidence Checklist](../docs/pr-evidence-checklist.md)
for detailed guidance.

### 1. Issue Reference
- [ ] The PR description links to the issue being addressed (e.g. `Fixes #123`).

### 2. Implementation Summary
- [ ] A concise summary of what was implemented, changed, or fixed is provided
      above in the Description section.
- [ ] Key files modified are listed with brief descriptions of each change.
- [ ] New public functions, events, error codes, or roles are documented.

### 3. Tests Added or Justification
- [ ] New or updated tests cover the change (happy path + failure paths).
- [ ] Test names and locations are listed (e.g. `test_mint_ok` in
      `src/test.rs:L45-60`).
- [ ] **OR** a [No-Test Justification](../docs/testing-standards.md#how-to-submit-a-no-test-justification)
      is provided and explicitly approved.

### 4. Commands Run
- [ ] `make verify` passes locally.
- [ ] Paste the relevant command output in the Additional Context section below.

### 5. CI Status
- [ ] All GitHub Actions checks pass (green) on the PR branch.
- [ ] If CI is failing, a clear explanation and link to the follow-up issue is
      provided.

### 6. Acceptance Criteria Coverage
- [ ] Every acceptance criterion from the issue is addressed in the Completion
      Table above.
- [ ] Incomplete criteria include a rationale and, where applicable, a link to
      a follow-up issue.

### Project Standards
- [ ] I have read the `CONTRIBUTING.md` guidelines.
- [ ] My code follows the Rust and Soroban formatting standards (ran `cargo fmt`).
- [ ] My changes generate no new warnings (ran `cargo clippy`).
- [ ] I have added/updated tests for the new logic, and all tests pass (ran `cargo test`).
- [ ] **Traceability Mapping:** I have filled out the detailed mapping table for any storage, event, or security changes.
- [ ] **Reviewer Guidance:** I have reviewed my own changes against the [Reviewer Checklist](../docs/reviewer-checklist.md).
- [ ] **Compliance & Legal Check:** I have verified that any new documentation or features do not imply regulatory completeness beyond smart contract enforcement, as per the [Legal Boundary Disclaimer](../docs/legal-boundary-disclaimer.md).

## Additional Context
<!-- Add any other context or screenshots about the pull request here. -->
