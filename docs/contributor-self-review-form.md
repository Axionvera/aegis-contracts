# Contributor Self-Review Form

**Use this form before requesting review or expecting payment approval.**

A complete self-review catches gaps early. Fill out every section honestly. If
you cannot check an item, note why — unresolved items are a flag for the
maintainer and will delay evaluation.

---

## 1. Requirements Review

- [ ] Every acceptance criterion from the issue is addressed (map them in your
      PR description via the [Traceability Mapping](./traceability-mapping.md)
      table).
- [ ] The implementation stays within the scope of the issue (no scope creep).
- [ ] Edge cases mentioned in the issue (zero amounts, max caps, unauthorized
      callers, paused state) are handled.
- [ ] The [Threat Model](./threat-model.md) and
      [Admin Misuse Risks](./admin-misuse-risks.md) were cross-checked for the
      affected area.

### Requirement gaps

| Criterion | Satisfied? | Notes |
| :--- | :---: | :--- |
| | Yes / No / Partial | |

---

## 2. Implementation Completeness

- [ ] The change implements **actual protocol behaviour** — not a stub,
      placeholder, or TODO comment.
- [ ] Authorization is enforced at every entry point (`require_auth`,
      `require_role`, `require_not_paused`).
- [ ] Numeric invariants are enforced (non-negative amounts, caps respected,
      no overflow/underflow paths).
- [ ] Compliance checks are applied where the spec requires them (whitelist
      checks in `transfer`/`mint`, eligibility checks).
- [ ] No privilege escalation: a scoped role cannot perform admin-only actions.
- [ ] No silent state changes — every state mutation emits its documented event
      ([Events](./events.md)).
- [ ] Failures return stable `Error` variants
      ([Error Codes](./error-codes.md)), never bare `panic!`/`assert!` strings.
- [ ] Input validation rejects invalid values early (negative amounts, zero
      addresses, out-of-range enums).
- [ ] Pause / emergency controls are respected where applicable
      ([Emergency Pause Policy](./emergency-pause.md)).
- [ ] The [Legal Boundary Disclaimer](./legal-boundary-disclaimer.md) is not
      contradicted by the implementation.

### Implementation gaps

| Area | Complete? | Notes |
| :--- | :---: | :--- |
| Protocol behaviour | Yes / No / Partial | |
| Authorization | Yes / No / Partial | |
| Security invariants | Yes / No / Partial | |
| Events | Yes / No / Partial | |
| Error codes | Yes / No / Partial | |
| Input validation | Yes / No / Partial | |

---

## 3. Testing Evidence

- [ ] **New tests exist** for every new function or behaviour path.
- [ ] **Failure paths are tested** — not just the happy path (unauthorized
      caller, paused contract, cap exceeded, invalid input).
- [ ] Tests assert both the returned value **and** the emitted event / error
      where relevant.
- [ ] **All tests pass** locally: `make test` is green.
- [ ] SDK integration fixtures are verified or updated if the change affects
      contract output: `make test-fixtures`.
- [ ] The safety matrix tests in `src/test.rs` pass.

### Test gaps

| Test area | Covered? | Test name(s) / file(s) |
| :--- | :---: | :--- |
| Happy path | Yes / No | |
| Failure paths | Yes / No | |
| Edge cases | Yes / No | |
| Event assertions | Yes / No | |

---

## 4. CI Status

- [ ] `make verify` passes locally
      ([Local Verification](./local-verification.md)):
      `fmt-check` + `clippy` + `test` + `build`.
- [ ] No `#[allow]` attributes hiding warnings.
- [ ] All remote CI checks are green (if already pushed).

### CI result (paste relevant output)

```
# paste `make verify` output here
```

---

## 5. Documentation

- [ ] `README.md` is updated if the change affects public interfaces,
      features, or setup steps.
- [ ] Relevant `docs/` pages are updated (events, error codes, admin roles,
      architecture, compliance lifecycle, etc.).
- [ ] Inline code comments are accurate and sufficient for non-obvious logic.
- [ ] The PR description includes a complete
      [Traceability Mapping](./traceability-mapping.md) table.
- [ ] The PR template checklist in
      [`.github/PULL_REQUEST_TEMPLATE.md`](../.github/PULL_REQUEST_TEMPLATE.md)
      is filled out.

### Documentation gaps

| Document | Updated? | Notes |
| :--- | :---: | :--- |
| `README.md` | Yes / No / N/A | |
| `docs/*` (specify) | Yes / No / N/A | |
| PR description | Yes / No | |

---

## 6. Known Limitations

List any limitations, deferred work, or conditions that reviewers should be
aware of:

| Limitation | Severity | Workaround / Future Work |
| :--- | :---: | :--- |
| | Low / Medium / High | |

Example limitations:
- A rare edge case is not handled (state why it is acceptable to defer).
- A test uses a simplified setup that does not reflect production configuration.
- A docs section was deferred for a follow-up PR.
- The change depends on an unmerged upstream dependency.

---

## 7. Final Declaration

- [ ] I have completed an honest self-review using the sections above.
- [ ] I would merge this PR myself if I were a maintainer.
- [ ] I understand that failing CI, incomplete testing, or missing
      documentation will delay review and may block payment approval.

**Contributor:** \_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_
**Date:** \_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_
**PR/Issue link:** \_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_\_
