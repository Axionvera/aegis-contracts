# PR Evidence Checklist

Every Pull Request must include a completed evidence checklist in its
description. This makes review objective by requiring explicit proof for each
acceptance criterion, test outcome, command run, and CI result.

## Evidence Checklist

### 1. Issue Reference
- [ ] The PR description links to the issue being addressed (e.g. `Fixes #123`
      or `Refs #456`).
- [ ] The PR title references the issue number when applicable.

### 2. Implementation Summary
- [ ] A concise summary of what was implemented, changed, or fixed.
- [ ] List of files modified with a brief description of each change (or attach
      the diff output).
- [ ] Any new public functions, events, error codes, or roles introduced are
      documented.

### 3. Tests Added or Justification
- [ ] New or updated tests cover the change (happy path + failure paths).
- [ ] Test names and their locations are listed (e.g. `test_mint_ok` in
      `src/test.rs:L45-60`).
- [ ] If no tests were added, a [No-Test Justification](./testing-standards.md#how-to-submit-a-no-test-justification)
      is provided and explicitly approved.

### 4. Commands Run
- [ ] Paste the output of `make verify` (or the relevant subset: `make fmt-check`,
      `make clippy`, `make test`, `make build`).
- [ ] If SDK fixtures were affected, paste `make test-fixtures` output.
- [ ] If integration or manual verification was performed, paste the commands
      and their output.

### 5. CI Status
- [ ] All GitHub Actions checks pass (green) on the PR branch.
- [ ] No `#[allow]` attributes introduced that suppress warnings.
- [ ] If CI is failing, a clear explanation and link to the follow-up issue is
      provided.

### 6. Acceptance Criteria Coverage
- [ ] Every acceptance criterion from the issue is addressed.
- [ ] The [Completion Table](./traceability-mapping.md#completion-table-format)
      in the PR description maps each criterion to its status
      (Complete/Partial/Not Met), implementation evidence, and test evidence.
- [ ] Incomplete criteria include a rationale and, where applicable, a link to
      a follow-up issue.

---

## How to Complete the Checklist

The PR evidence checklist is embedded directly in the
[PR template](../.github/PULL_REQUEST_TEMPLATE.md) as the `## PR Evidence
Checklist` section. Fill out every applicable checkbox and paste command output
before requesting review.

### Quick reference (copy-paste the output):

```bash
# Run this before opening your PR:
make verify

# If you changed contract output that SDKs consume:
make test-fixtures
```

## Reviewing the Evidence

Maintainers should verify each item:

| Item | What to Check |
| :--- | :--- |
| Issue Reference | The linked issue exists and matches the PR scope |
| Implementation Summary | The summary accurately describes the diff |
| Tests | Tests exist, pass, and cover the change |
| Commands Run | The output shows no errors or warnings |
| CI Status | The CI badge or screenshot shows green |
| Acceptance Criteria | Every criterion in the issue is mapped in the Completion Table |

If any item is missing or insufficient, request clarification before proceeding
with a full code review.
