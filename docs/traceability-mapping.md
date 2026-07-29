# Requirement Traceability Mapping

To ensure high-quality engineering and evaluation readiness, all contract changes must be mapped to their respective acceptance criteria. This documentation defines the standard traceability table format required for all Pull Requests (PRs).

The Completion Table is the canonical acceptance criteria audit template used
for every PR.

## Completion Table Format

Every PR must include a completion table in its description mapping each acceptance criterion to its status and evidence. This table helps contributors self-evaluate before payment day and makes review expectations clear.

### Table Structure

| Acceptance Criterion | Status | Implementation Evidence | Test Evidence | Documentation Impact |
| :--- | :---: | :--- | :--- | :--- |
| **AC 1**: Brief description | Complete | `fn name()` — link to line or brief description | `test_name` — file:line | `docs/page.md` updated |
| **AC 2**: ... | Partial | Reason and what was done | Tests that exist | Follow-up docs needed |
| **AC 3**: ... | Not Met | Explanation | N/A | N/A — no behavior changed |

### Status Definitions

| Status | Meaning |
| :--- | :--- |
| **Complete** | The criterion is fully satisfied — implementation and tests are in place. |
| **Partial** | The criterion is partially addressed but has known gaps (must explain why in the evidence column). |
| **Not Met** | The criterion is not addressed in this PR (must explain why in the evidence column). |

### Documentation Impact Evidence

For every criterion, use the **Documentation Impact** column to identify the
README, contributor guide, API reference, specification, or other document
that changed. Write `N/A` with a short reason when the criterion has no
documentation impact. Do not leave the column blank: an explicit `N/A` tells
reviewers that documentation impact was assessed rather than overlooked.

### Handling Incomplete Criteria

If any acceptance criterion is marked **Partial** or **Not Met**:

1. **Explain why** in the Implementation Evidence column (e.g., "scope limited — criterion deferred to follow-up PR", "blocked by upstream dependency", "out of scope for this issue").
2. **Link to a follow-up issue** if the criterion is intentionally deferred.
3. **Record documentation impact** for the work that is complete and note any
   deferred documentation in the Documentation Impact column.
4. **Do not request payment evaluation** for incomplete criteria — payment is assessed against full acceptance criteria coverage.
5. Reviewers may reject a PR with incomplete criteria unless a clear, accepted rationale is provided and the incomplete items are explicitly scoped out.

> If you cannot mark all criteria Complete, be honest about it. Incomplete criteria with clear explanations and follow-up plans are accepted more readily than claims of completeness that don't hold up under review.

---

## Detailed Traceability Table Format (Advanced)

For complex PRs involving storage changes, events, or security controls, include the detailed traceability table below instead of (or in addition to) the completion table.

### Table Structure

| Acceptance Criteria | Implementation (Functions/Logic) | Storage & State Changes | Events Emitted | Test Coverage (File:Line) | Security/Safety Controls |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **AC 1**: Brief description | `fn name()` | `DataKey::X` (Instance/Persistent) | `EventName` | `test_name` | `require_auth`, `require_not_paused` |
| **AC 2**: ... | ... | ... | ... | ... | ... |

---

## Mapping Definitions

1.  **Acceptance Criteria**: The specific requirement being addressed, as defined in the issue.
2.  **Implementation**: The specific function(s) or logic blocks that implement the requirement.
3.  **Storage & State Changes**: Any new or modified `DataKey` entries, specifying if they use `Instance`, `Persistent`, or `Temporary` storage.
4.  **Events Emitted**: The `contracttype` events published upon successful execution.
5.  **Test Coverage**: The specific unit test name and location (file and approximate line range) verifying the logic.
6.  **Security/Safety Controls**: Authorization checks (`require_auth`), pause checks (`require_not_paused`), or role-based access control (`require_role`) applied to the logic.

---

## Example Mapping

*Scenario: Implementing a new supply cap amendment flow.*

| Acceptance Criteria | Implementation | Storage & State Changes | Events Emitted | Test Coverage | Security/Safety Controls |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **AC 1**: Only admin can propose cap | `propose_supply_cap` | `DataKey::SupplyCapCandidate` (Instance) | `SupplyCapProposedEvent` | `test.rs:L145-160` | `require_auth`, `get_admin` check |
| **AC 2**: Cap must be non-negative | `propose_supply_cap` | N/A (Validation) | N/A | `test.rs:L162-170` | `assert!(proposed_cap >= 0)` |
| **AC 3**: Amendments are 2-step | `accept_supply_cap` | `DataKey::SupplyCap` (Instance) | `SupplyCapAmendedEvent` | `test.rs:L175-195` | `require_not_paused`, `require_auth` |

## Why This Matters

- **Audit Readiness**: Provides a clear roadmap for security auditors to verify that every requirement has a corresponding implementation and test.
- **Review Efficiency**: Helps maintainers quickly locate the relevant logic and tests during the PR review process.
- **Protocol Stability**: Ensures no edge cases or security controls are missed during implementation.
- **Payment Readiness**: Enables contributors to self-evaluate before payment day — a complete table with all criteria marked Complete gives reviewers confidence to approve payment.
