# Aegis Contracts — Meaningful Change Threshold Guide

Guidance for contributors and reviewers on how to judge whether a PR is
**substantively done**, independent of how many lines it touches.

> This guide answers one question: *"Is this change enough to close the
> issue?"* For the technical bar a contract change must clear (authorization,
> events, error codes, tests), see the
> [Meaningful Implementation Checklist](meaningful-implementation-checklist.md).
> This guide focuses on **scope judgment** — the line-count fallacy, and how
> to tell a small-but-complete fix apart from a small-but-incomplete one.

## Why this exists

Some PRs land with very small diffs that don't solve the underlying issue —
a renamed variable, a comment, a single test, or a partial fix that leaves
the reported behaviour unfixed. Line count is not a reliable signal either
way:

- A **one-line fix** can fully resolve an issue (e.g. adding a missing
  `require_role` check).
- A **300-line PR** can still leave the issue open (new module, no tests, no
  enforcement of the actual invariant reported).

**The standard is completeness against the issue, not diff size.**

## The threshold: three questions

A change clears the meaningful-change threshold only if the answer to all
three is yes:

1. **Does it solve the reported problem**, not a symptom or a piece of it?
2. **Is it verified** — tests that would fail without the change, and pass
   with it (see [Testing Standards](testing-standards.md))?
3. **Is it traceable to the issue's acceptance criteria**, with every
   criterion met or explicitly called out as deferred (see
   [Requirement Traceability Mapping](traceability-mapping.md))?

If any answer is no, the change is incomplete regardless of its size.

## Small but complete vs. small but incomplete

Both examples below are ~2 lines. Only one is meaningful.

**Small and complete** — closes the issue outright:
```rust
// Issue: unauthorized callers can update the holding cap.
pub fn set_holding_cap(env: Env, admin: Address, cap: i128) -> Result<(), Error> {
    require_role(&env, &admin, Role::AssetManager)?; // <-- the fix
    if cap < 0 {
        return Err(Error::InvalidAmount);
    }
    env.storage().instance().set(&HOLDING_CAP, &cap);
    env.events().publish(("holding_cap_updated",), (admin, cap));
    Ok(())
}
```
This is a tight diff, but it enforces the missing invariant, keeps the
existing event and error handling, and is covered by a test that proves a
non-admin caller is rejected. Nothing further is needed to close the issue.

**Small and incomplete** — looks similar, doesn't close the issue:
```rust
// Issue: unauthorized callers can update the holding cap.
pub fn set_holding_cap(env: Env, admin: Address, cap: i128) {
    // TODO: add auth check once role system is finalized
    env.storage().instance().set(&HOLDING_CAP, &cap);
}
```
Same line count, but the actual vulnerability (missing authorization) is
untouched — it's deferred behind a comment. No test proves rejection because
nothing is rejected. This does not clear the threshold no matter how "clean"
the diff looks.

The distinguishing factor is never size — it's whether the reported problem
is actually gone and proven gone by a test.

## Examples of insufficient changes

These patterns show up as real diffs but do not meet the threshold, even
when merged:

- **Cosmetic-only changes** — renaming variables, reformatting, or comment
  edits presented as a fix for a behavioural issue.
- **Partial fixes** — handling one of several reported cases (e.g. rejecting
  a negative amount but not a zero address) while marking the issue resolved.
- **Tests added without a corresponding fix** — a new test that documents
  the bug but doesn't assert the corrected behaviour, or a test that passes
  whether or not the fix is present.
- **Suppressing instead of fixing** — silencing a warning, adding `#[allow]`,
  or catching an error without addressing why it occurs.
- **Scope narrowed silently** — implementing only the easy half of the issue
  and not disclosing the remainder as out-of-scope in the PR description or
  [Completion Table](traceability-mapping.md#completion-table-format).
- **Documentation-only response to a behavioural issue** — updating a doc
  comment to describe the bug instead of fixing the code.

For a broader gallery of failure categories with side-by-side comparisons,
see [Aegis Contracts Contribution Examples](aegis-contracts-examples.md).

## Reviewer assessment guidance

When reviewing a PR, don't gauge effort by diff size. Instead:

- [ ] Read the linked issue's acceptance criteria first, then check each one
      off against the diff — not the PR description's claims.
- [ ] Ask: "If I revert just this diff, does the original bug/gap come back?"
      If yes, the diff is load-bearing. If the repo behaves the same either
      way, the diff isn't meaningful yet.
- [ ] Check that new/changed tests actually fail on the pre-change code
      (mentally or by checking out `main` and running them).
- [ ] Look for TODOs, `#[allow(...)]`, or deferred-work comments standing in
      for the real fix.
- [ ] Confirm partial scope is disclosed, not silently merged as "done."
- [ ] Weigh a large diff the same way — more code is not evidence of more
      correctness. Apply the same three questions from
      [above](#the-threshold-three-questions).

A PR that fails this assessment is not meaningful yet, regardless of size —
request the missing behaviour, test, or disclosure before approval.

## Related Documents

- [Meaningful Implementation Checklist](meaningful-implementation-checklist.md) — the technical completeness bar (auth, invariants, events, errors, tests)
- [Aegis Contracts Contribution Examples](aegis-contracts-examples.md) — side-by-side comparisons of low-effort, partial, and acceptable contributions
- [Requirement Traceability Mapping](traceability-mapping.md) — mandatory completion table format for acceptance criteria
- [Reviewer Checklist](reviewer-checklist.md) — standardized quality and security checklist for PR reviewers
- [Contributor Evaluation Policy](contributor-evaluation-policy.md) — formal policy this guide supports
