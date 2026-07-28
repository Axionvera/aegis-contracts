# Evaluation Readiness Summary

**One central page for contributors to check before payment day.**

This page summarizes what makes a contribution evaluation-ready for Aegis Contracts. Before submitting your work for review or payment evaluation, ensure you've completed all items below.

---

## Quick Pre-Submission Checklist

Before you consider your contribution "done," verify:

- [ ] **Code builds and tests pass locally** — run `make verify` (see [Local Verification](#local-verification))
- [ ] **All acceptance criteria are satisfied** — mapped in your PR description (see [Acceptance Criteria Mapping](#acceptance-criteria-mapping))
- [ ] **Implementation is meaningful** — not just stubs or comments (see [Meaningful Implementation Checklist](#meaningful-implementation-checklist))
- [ ] **PR description is complete** — includes traceability table and context (see [PR Evidence Checklist](#pr-evidence-checklist))
- [ ] **You've self-reviewed** — complete the [Contributor Self-Review Form](./contributor-self-review-form.md) (see [Self-Review](#self-review))
- [ ] **CI checks are green** — no failing automation (see [CI Workflow](#ci-workflow))

---

## 1. Local Verification

**Run this single command before pushing:**

```bash
make verify
```

`make verify` runs the complete pre-push gate:
- `cargo fmt --all -- --check` — formatting check
- `cargo clippy --all-targets -- -D warnings` — lint check
- `cargo test` — full test suite
- `cargo build --target wasm32v1-none --release` — WASM build

All four must pass locally before opening or updating a PR.

📖 **Full guide:** [Local Verification Command](./local-verification.md)

---

## 2. CI Workflow

This repo uses external automation via `Axionvera/pocketpay-issue-automation` for CI checks. A red check is a blocker for approval.

### If CI fails:
1. Reproduce locally with `make build` and `make test`
2. Fix the root cause (don't mask warnings/errors)
3. Re-run locally until fully green
4. Push your fix — automation re-runs on new commits
5. Confirm checks flip to green before requesting review

### Common failure categories:
- **Rust compilation** — missing WASM target, SDK drift, warnings as errors
- **Soroban failures** — wrong binary name/path, contract logic panics
- **Makefile issues** — tab vs spaces, missing tools on PATH
- **Dependency failures** — pinned SDK missing, Cargo.lock drift
- **Workflow dispatch** — token/permission issues (flag to maintainer)

📖 **Full guide:** [Failing CI Response Guide](./failing-ci-guide.md)

---

## 3. PR Evidence Checklist

Your PR description must include:

### Required elements:
- [ ] **Description** — clear explanation of what changed and why
- [ ] **Related Issues** — link to the issue this PR addresses (e.g., `Fixes #123`)
- [ ] **Completion Table** — maps every acceptance criterion to a status (Complete/Partial/Not Met), implementation evidence, and test evidence (see [Completion Table Format](./traceability-mapping.md#completion-table-format))
- [ ] **Traceability Mapping Table** — detailed map of acceptance criteria to implementation, storage changes, events, tests, and security controls (for complex PRs)
- [ ] **Type of Change** — select appropriate category (bug fix, new feature, breaking change, documentation, chore)
- [ ] **Review Checklist** — all items checked (see below)

### Review Checklist items:
- [ ] Read `CONTRIBUTING.md` guidelines
- [ ] Code follows Rust and Soroban formatting standards (`cargo fmt`)
- [ ] No new warnings (`cargo clippy`)
- [ ] Added/updated tests for new logic, all tests pass (`cargo test`)
- [ ] **Completion Table** filled out with status per criterion
- [ ] **Traceability Mapping** filled out in PR description
- [ ] Reviewed against [Reviewer Checklist](./reviewer-checklist.md)
- [ ] Verified compliance with [Legal Boundary Disclaimer](./legal-boundary-disclaimer.md)

📖 **PR Template:** [`.github/PULL_REQUEST_TEMPLATE.md`](../.github/PULL_REQUEST_TEMPLATE.md)

---

## 4. Acceptance Criteria Mapping

Every PR must include a completion table in its description mapping each acceptance criterion to its status and evidence. For complex PRs, also include a detailed traceability table.

### Completion Table format:

| Acceptance Criterion | Status | Implementation Evidence | Test Evidence |
| :--- | :---: | :--- | :--- |
| **AC 1**: Brief description | Complete | `fn name()` — link or description | `test_name` — file:line |
| **AC 2**: ... | Partial | Reason and what was done | Tests that exist |
| **AC 3**: ... | Not Met | Explanation | N/A |

**Status** must be one of: **Complete** (fully satisfied), **Partial** (known gaps with explanation), or **Not Met** (not addressed with rationale).

### Detailed Traceability Table (for complex PRs):

| Acceptance Criteria | Implementation (Functions/Logic) | Storage & State Changes | Events Emitted | Test Coverage (File:Line) | Security/Safety Controls |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **AC 1**: Brief description | `fn name()` | `DataKey::X` (Instance/Persistent) | `EventName` | `test_name` | `require_auth`, `require_not_paused` |
| **AC 2**: ... | ... | ... | ... | ... | ... |

### Why this matters:
- **Audit Readiness** — clear roadmap for security auditors
- **Review Efficiency** — helps maintainers locate relevant logic and tests
- **Protocol Stability** — ensures no edge cases or security controls are missed
- **Payment Readiness** — enables contributors to self-evaluate before payment day

📖 **Full guide:** [Requirement Traceability Mapping](./traceability-mapping.md) — includes completion table format, status definitions, and incomplete criteria handling

---

## 5. Meaningful Implementation Checklist

A contribution is meaningful when it changes **contract behavior** in a way that is:

1. **Protocol-correct** — matches the documented spec and compliance model
2. **Secure by construction** — enforces authorization, bounds, and invariants
3. **Observable** — emits the right events and returns stable error codes
4. **Verified** — covered by tests that prove the behavior, including failures
5. **Acceptance-driven** — satisfies the issue's stated criteria, not a subset

### Before you start:
- [ ] Read relevant spec docs (contract-spec, admin-roles, compliance-registry-reads, etc.)
- [ ] Identify which contract module owns the behavior
- [ ] Note the authorization model (who may call this?)
- [ ] Note relevant events and error codes

### Implementation standards:
- [ ] Protocol behavior enforced (actual rule, not stub)
- [ ] Authorization enforced at entry point
- [ ] Numeric invariants hold (non-negative amounts, caps respected)
- [ ] Compliance enforced where spec requires it
- [ ] No privilege escalation or unauthorized state mutation
- [ ] Pause/emergency controls respected
- [ ] Input validation rejects invalid values early
- [ ] State-changing actions emit documented events
- [ ] Failures use stable `Error` variants, not panic strings
- [ ] New behavior has tests (including failure paths)
- [ ] All issue acceptance criteria satisfied

📖 **Full guide:** [Meaningful Implementation Checklist](./meaningful-implementation-checklist.md)

---

## 6. Self-Review

Before considering your contribution "done", complete the
**[Contributor Self-Review Form](./contributor-self-review-form.md)** — a
structured checklist covering requirements, implementation completeness,
testing evidence, CI status, documentation, and known limitations.

### Quick quality checks (supplement to the form):
- [ ] Does it actually satisfy the issue's acceptance criteria?
- [ ] Do `make build` and `make test` pass locally?
- [ ] Is the code minimal, readable, and free of `#[allow]`-hidden warnings?
- [ ] Did you update docs/README if the change affects them?
- [ ] Is the PR description clear about what changed and why?

### The "merge test":
**If you wouldn't merge it yourself, don't expect a maintainer to.**

---

## 7. Payment-Period Conduct Guidance

If you're contributing during a paid period (e.g., GrantFox campaign):

### Key principles:
- **Labels are not promises** — a `Maybe Rewarded` label is discretionary, not a commitment
- **Avoid spamming community channels** — ask payment questions once, clearly, then wait
- **Self-review honestly** — quality matters more than PR count
- **Be patient and professional** — evaluation queues have many contributors

### GrantFox evaluation process:
1. **Maintainer assignment** — assesses whether work meets the issue's bar
2. **Program approval** — GrantFox admin approves against campaign budget and caps
3. **Payout** — released after campaign ends via Stellar USDC escrow

**A merged PR does not equal immediate or guaranteed payment.** This is by design.

📖 **Full guide:** [Payment-Period Conduct Note](./payment-period-conduct.md)

---

## 8. Additional Resources

### Core documentation:
- [CONTRIBUTING.md](../CONTRIBUTING.md) — development workflow, branching, testing requirements
- [Local Deployment Guide](./local-deployment.md) — environment setup, Makefile reference, Soroban CLI usage
- [Reviewer Checklist](./reviewer-checklist.md) — standardized quality and security checklist for PR reviewers

### Security & compliance:
- [Threat Model](./threat-model.md) — protected assets, trust boundaries, threat catalog
- [Admin Roles & Permissions](./admin-roles.md) — role-based access control design
- [Compliance Status Lifecycle](./compliance-lifecycle.md) — investor lifecycle and enforcement
- [Emergency Pause Policy](./emergency-pause.md) — global pause mechanism and authorization

### Testing & verification:
- [Minimum Testing Standards](./testing-standards.md) — **mandatory** testing requirements per module, happy-path and negative-path expectations, integration fixtures, and no-test justification policy
- [Failing CI Response Guide](./failing-ci-guide.md) — how to reproduce and fix automation failures
- [Local Verification Command](./local-verification.md) — single pre-push command

### Contributor experience:
- [Contributor Experience Review](./contributor-experience-review.md) — known onboarding friction and follow-up items

---

## Summary: The Evaluation-Ready Contribution

An evaluation-ready contribution is one where:

1. ✅ **Code works** — builds, tests pass, CI is green
2. ✅ **Behavior is meaningful** — implements actual protocol rules with security
3. ✅ **Evidence is complete** — traceability table, clear PR description, all checklists filled
4. ✅ **Self-review passed** — you would merge this yourself
5. ✅ **Conduct is professional** — respectful communication, no spamming

**Use this page as your pre-submission checklist.** It gives you one clear place to verify your work is ready for evaluation and payment consideration.
