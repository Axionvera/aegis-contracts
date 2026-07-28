# Reviewer Checklist

This document provides a standardized checklist for reviewers of the Aegis RWA smart contracts. Reviewers should ensure that every Pull Request (PR) maintains the protocol's high standards for security, compliance, and engineering excellence.

## 1. Requirement & Traceability Verification
- [ ] **PR Evidence Checklist**: Has the contributor completed the [PR Evidence Checklist](../docs/pr-evidence-checklist.md) in the PR description, covering issue reference, implementation summary, tests, commands run, CI status, and acceptance criteria coverage?
- [ ] **Acceptability Criteria**: Does the PR fully satisfy the [Acceptance Criteria] defined in the linked issue?
- [ ] **Traceability Table**: Has the contributor provided a complete [Requirement Traceability Mapping](../docs/traceability-mapping.md)?
- [ ] **Scope**: Does the implementation stay within the scope of the issue, avoiding "scope creep" or unrelated changes?

## 2. Functional & Behavioral Logic
- [ ] **Core Logic**: Does the implementation correctly reflect the intended business logic (e.g., minting, transfers, cap amendments)?
- [ ] **Edge Cases**: Are boundary conditions (zero amounts, max balances, unauthorized callers) handled correctly?
- [ ] **Pause Safety**: Are all state-changing operations protected by `require_not_paused` where appropriate?
- [ ] **Read Helpers**: If new state is added, is there a corresponding read helper or an update to [Investor Eligibility](../docs/investor-eligibility.md)?

## 3. Security & Access Control
- [ ] **Authorization**: Is `require_auth()` called for all sensitive operations?
- [ ] **RBAC**: Does the code use `require_role` or `require_any_role` correctly according to the [Admin Roles](../docs/admin-roles.md) policy?
- [ ] **Attack Surface**: Does the change introduce any new attack vectors (e.g., reentrancy risks, unauthorized state mutation, overflow vulnerabilities)?
- [ ] **Administrative Misuse**: Does the change align with the [Admin Misuse Risks](../docs/admin-misuse-risks.md) documentation?

## 4. Tests & CI Verification
- [ ] **Test Coverage**: Are there new unit tests in `src/test.rs` covering both happy paths and failure paths?
- [ ] **Deterministic Results**: Do all tests pass consistently (ran `make test`)?
- [ ] **CI Pipeline**: Have all GitHub Action checks (formatting, clippy, build, tests) passed?
- [ ] **Safety Matrix**: If core logic changed, was the safety matrix test in `src/test.rs` updated to reflect the new invariants?

## 5. Events & Storage Mapping
- [ ] **Event Emission**: Are structured events published for every successful state change?
- [ ] **Storage Class**: Is the correct storage class used (Instance vs. Persistent) as per the [Storage Audit Map](../docs/storage-audit-map.md)?
- [ ] **Data Keys**: Are new `DataKey` variants added to the central enum in `lib.rs`?
- [ ] **Event Types**: Are event payloads correctly typed and do they include necessary metadata (caller, amount, previous/new state)?

## 6. Documentation & Quality
- [ ] **Standardized Errors**: Does the code use the numeric error ranges defined in [Error Codes](../docs/error-codes.md)?
- [ ] **Code Clarity**: Is the code self-documenting, with clear naming conventions and helpful comments for complex logic?
- [ ] **System Overview**: Does the [System Overview](../docs/system-overview.md) or [Technical Report](../docs/technical-report.md) need updating based on these changes?
- [ ] **Legal Disclaimer**: Does the PR avoid making regulatory claims beyond smart contract enforcement?

---

## Review Process

1. **Static Analysis**: Review the code for logic, security, and standards.
2. **Local Verification**: Pull the branch and run `make ci` locally.
3. **Traceability Audit**: Cross-reference the implementation against the traceability table.
4. **Approval**: Only approve if all checklist items are satisfied or explicitly waived with justification.
