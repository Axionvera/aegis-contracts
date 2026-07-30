# Aegis Contracts — Contributor Self-Assessment Form

Please complete this self-assessment form prior to submitting a pull request or requesting review/payment. This ensures ang work is verified objectively against project standards.

---

## 1. Scope Confirmation
- [ ] **Scope Match:** The implemented changes strictly address the goals defined in the issue.
- [ ] **No Unrelated Code:** Unrelated refactoring, dependency changes, or formatting updates were avoided.
- [ ] **Minimal Diff:** Every modified file is strictly required for this contribution.

---

## 2. Test Evidence
- [ ] **Unit & Integration Coverage:** New tests cover happy paths, edge cases, and failure modes.
- [ ] **Local Passing Suite:** All unit and integration tests pass locally without errors.
- [ ] **Execution Summary:**

'`'text
<!-- Paste local test command output summary here (e.g., yarn test / forge test) -->
'`'

---

## 3. CI Status & Code Quality
- [ ] **CI Pipeline:** All automated checks, linters, and build pipelines pass cleanly.
- [ ] **Linting & Formatting:** Code adheres to repository style rules (`eslint`, `prettier`, or `forge fmt`).
- [ ] **Zero New Warnings:** No new compiler warnings, deprecations, or static analysis flags were introduced.

---

## 4. Known Limitations & Trade-offs
- [ ] **Documented Limitations:** Known edge cases, unhandled paths, or performance trade-offs are clearly described below:
  - *List limitations here (or write "None identified").*
- [ ] **Technical Debt:** Any temporary workarounds or follow-up tasks have open tracking issues linked.

---

## 5. Acceptance Criteria Confirmation
- [ ] **Criteria Verified:** Every explicit acceptance criterion in the target issue has been met and re-verified.
- [ ] **Documentation Updated:** Relevant specifications, README files, or inline comments have been updated.
- [ ] **Line-by-Line Review:** I have performed a manual self-review of my entire diff prior to submitting.

---

**Contributor Sign-off:**
- **Author:** `@username`
- **Date:** `YYYY-MM-DD`