# Requirement Traceability Mapping

To ensure high-quality engineering and evaluation readiness, all contract changes must be mapped to their respective acceptance criteria. This documentation defines the standard traceability table format required for all Pull Requests (PRs).

## Traceability Table Format

Every non-trivial PR must include a traceability table in its description. This table maps how each requirement (Acceptance Criteria) from the related issue is satisfied within the codebase.

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
