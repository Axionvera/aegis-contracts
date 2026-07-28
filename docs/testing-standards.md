# Minimum Testing Standards for Aegis Contracts Contributions

This document defines the minimum testing standard every PR must meet before
merge. It applies to all changes in the contract source (`src/`), tests
(`tests/`), and the supporting SDK fixture layer (`fixtures/sdk/`).

---

## Table of Contents

- [Why This Standard Exists](#why-this-standard-exists)
- [General Rules](#general-rules)
- [Testing Expectations by Module](#testing-expectations-by-module)
  - [Compliance Registry](#compliance-registry)
  - [RWA Assets (Minting, Burning, Yield)](#rwa-assets-minting-burning-yield)
  - [Transfer Restrictions](#transfer-restrictions)
  - [Roles & Admin Governance](#roles--admin-governance)
  - [Metadata](#metadata)
  - [Contract Events](#contract-events)
- [Happy-Path Testing Expectations](#happy-path-testing-expectations)
- [Negative-Path Testing Expectations](#negative-path-testing-expectations)
- [Integration & Fixture Tests](#integration--fixture-tests)
- [Manual Verification](#manual-verification)
- [No-Test Justification Guidance](#no-test-justification-guidance)
- [Running Tests](#running-tests)

---

## Why This Standard Exists

Merged PRs without adequate tests erode confidence in the protocol. Every
change to contract behaviour — whether it adds a feature, fixes a bug, or
refactors logic — must be accompanied by tests that prove:

- The intended behaviour works (**happy path**).
- Invalid inputs, unauthorized callers, and illegal state transitions are
  rejected (**negative path**).
- State invariants hold before and after every operation.
- Events are emitted with the correct topic and payload.
- No regressions are introduced in unchanged modules.

This standard is **mandatory**. PRs that do not meet it should be sent back
for additional tests before review.

---

## General Rules

1. **Every state-changing function** must have at least one happy-path test
   and at least one negative-path test (unauthorized caller, invalid input,
   or illegal state).

2. **Every event** must have a test that asserts its exact topic and payload
   shape using `env.events().all()`. See existing examples in
   [`src/test.rs`](../src/test.rs).

3. **Every error code** returned by a function must be exercised by at least
   one test that asserts `Err(Ok(Error::TheVariant))`.

4. **Read-only functions** that return computed state must have at least one
   test proving the returned value is correct under a known setup.

5. **State invariants** must be asserted after every state-changing operation
   in tests (e.g., "total supply equals sum of balances", "cap has not been
   exceeded").

6. **CI must pass.** Run `make verify` (fmt-check + clippy + test + build)
   before pushing.

---

## Testing Expectations by Module

### Compliance Registry

| Scenario | Test Type | What to Assert |
|----------|-----------|----------------|
| Whitelist a user | Happy-path | Status transitions to `Approved`, event emitted |
| Revoke whitelist | Happy-path | Status transitions to `Revoked`, event emitted |
| Set compliance status | Happy-path | Correct `previous_status`/`new_status` in event |
| Re-approve an already-approved user | Negative-path | Idempotent — no duplicate lifecycle event |
| Revoke an unknown user | Negative-path | Tolerated no-op, only legacy event emitted |
| Unauthorized caller tries to whitelist | Negative-path | `Error::Unauthorized` |
| Whitelist while paused | Negative-path | `Error::ContractPaused` |
| Status transition matrix | Integration | Every allowed transition succeeds; every disallowed transition fails |
| Compliance registry reads | Happy-path | `is_whitelisted`, `get_compliance_status` return correct values |
| Reads after state change | Negative-path | Retrieving status for an unregistered address returns `Unknown` |

### RWA Assets (Minting, Burning, Yield)

| Scenario | Test Type | What to Assert |
|----------|-----------|----------------|
| Mint to a whitelisted recipient | Happy-path | Balance increases, total supply increases, event emitted |
| Mint with admin role | Happy-path | Admin can mint without explicit AssetManager role |
| Mint when asset is Active | Happy-path | Succeeds |
| Mint zero amount | Negative-path | `Error::InvalidAmount`, no state change |
| Mint to non-whitelisted recipient | Negative-path | `Error::ReceiverNotWhitelisted` |
| Mint without AssetManager role | Negative-path | `Error::Unauthorized` |
| Mint with wrong role (e.g. ComplianceOfficer) | Negative-path | `Error::Unauthorized` |
| Mint while paused | Negative-path | `Error::ContractPaused` |
| Mint when asset is not Active | Negative-path | Appropriate asset-status error |
| Mint above supply cap | Negative-path | `Error::SupplyCapExceeded`, no state change |
| Mint exactly at supply cap boundary | Happy-path | Succeeds |
| Distribute yield with AssetManager role | Happy-path | Event emitted |
| Distribute yield without role | Negative-path | `Error::Unauthorized` |
| Distribute yield zero amount | Negative-path | `Error::InvalidAmount` |
| Distribute yield while paused | Negative-path | `Error::ContractPaused` |

### Transfer Restrictions

| Scenario | Test Type | What to Assert |
|----------|-----------|----------------|
| Transfer between whitelisted addresses | Happy-path | Balances update, event emitted |
| Transfer with sufficient balance | Happy-path | Succeeds |
| Transfer zero amount | Negative-path | `Error::InvalidAmount` |
| Transfer when sender not whitelisted | Negative-path | `Error::SenderNotWhitelisted` |
| Transfer when receiver not whitelisted | Negative-path | `Error::ReceiverNotWhitelisted` |
| Transfer with insufficient balance | Negative-path | `Error::InsufficientBalance` |
| Transfer while paused | Negative-path | `Error::ContractPaused` |
| Transfer when asset is Retired | Negative-path | `Error::AssetRetiredRestriction` |
| Transfer when asset is Blocked | Negative-path | `Error::AssetNotActive` |
| Transfer exceeding holding cap | Negative-path | `Error::HoldingCapExceeded`, no balance change |
| Transfer at exact holding cap boundary | Happy-path | Succeeds |
| `check_transfer_eligibility` returns all applicable restrictions | Happy-path | Each restriction reason is correctly returned |
| Eligibility check with no restrictions | Happy-path | Returns `eligible: true` |
| Transfer restriction reason codes | Integration | `reason_for_error` / `error_for_reason` mappings are bijective |

### Roles & Admin Governance

| Scenario | Test Type | What to Assert |
|----------|-----------|----------------|
| Assign a role to a user | Happy-path | Role is set, event emitted |
| Remove a role from a user | Happy-path | Role returns to `None`, event emitted |
| Query role of assigned user | Happy-path | Returns correct role |
| Query role of unassigned user | Happy-path | Returns `Role::None` |
| Set role by non-admin | Negative-path | `Error::Unauthorized` |
| Remove role by non-admin | Negative-path | `Error::Unauthorized` |
| Remove role from user with no role | Negative-path | `Error::NoRoleToRevoke` |
| Assign Admin role via `set_role` | Negative-path | `Error::CannotAssignAdminRole` |
| Set/remove role while paused | Negative-path | `Error::ContractPaused` |
| Initiate admin transfer | Happy-path | Pending candidate set, event emitted |
| Accept admin transfer as correct candidate | Happy-path | Admin role transferred, event emitted |
| Accept admin transfer as wrong candidate | Negative-path | `Error::NotPendingCandidate` |
| Accept admin transfer with no pending transfer | Negative-path | `Error::NoPendingAdminTransfer` |
| Renounce admin | Happy-path | Admin removed, event emitted |
| Renounce admin by non-admin | Negative-path | `Error::Unauthorized` |

### Metadata

| Scenario | Test Type | What to Assert |
|----------|-----------|----------------|
| Update asset metadata (name, symbol, uri) | Happy-path | Values stored, event emitted |
| Update metadata by unauthorized caller | Negative-path | `Error::Unauthorized` |
| Update metadata while paused | Negative-path | `Error::ContractPaused` |
| Query metadata after update | Happy-path | Read helpers return updated values |
| Query metadata before any update | Happy-path | Read helpers return defaults or empty values |

### Contract Events

| Scenario | Test Type | What to Assert |
|----------|-----------|----------------|
| Every documented event is emitted on the correct action | Happy-path | Topic string matches `docs/events.md`, payload fields are correct |
| Reverted invocation emits no events | Negative-path | `env.events().all().events().len() == 0` after a failed call |
| Event `caller` is the authorizing address | Happy-path | Field matches the caller argument, not a hardcoded address |
| Event payload includes all documented fields | Happy-path | Every field in the payload struct is populated and non-default where expected |

---

## Happy-Path Testing Expectations

A happy-path test proves the intended behaviour works under ideal conditions.
Every state-changing function must have at least one happy-path test.

**Happy-path tests must assert:**

- The function returns `Ok(())` or the expected success value.
- The relevant storage state is correctly mutated (balance, status, cap, etc.).
- The correct event is emitted with the correct topic and payload.
- No unexpected side effects occurred in unrelated storage keys.

**Example structure:**

```rust
#[test]
fn test_whitelist_user_ok() {
    let (env, client, admin, user, _) = setup_active();

    let result = client.try_whitelist_user(&admin, &user);
    assert!(result.is_ok());
    assert!(client.is_whitelisted(&user));
    assert_eq!(client.get_compliance_status(&user), ComplianceStatus::Approved);

    // Assert event shape
    assert_eq!(
        env.events().all(),
        vec![&env, (client.address.clone(), ("compliance_status_changed",).into_val(&env), ...)]
    );
}
```

---

## Negative-Path Testing Expectations

A negative-path test proves the contract correctly rejects invalid operations.
Every error code a function can return must be exercised by at least one test.

**Negative-path tests must assert:**

- The function returns the expected `Error` variant.
- **No state mutation occurred** — balances, statuses, caps, and other
  storage values are identical to before the call.
- No events were emitted (Soroban discards events from reverted calls).

**Example structure:**

```rust
#[test]
fn test_whitelist_reverts_when_not_authorized() {
    let (env, client, admin, user, caller) = setup_active();

    // Snapshot pre-state
    let pre_whitelisted = client.is_whitelisted(&user);

    let result = client.try_whitelist_user(&caller, &user);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));

    // Assert state is unchanged
    assert_eq!(client.is_whitelisted(&user), pre_whitelisted);
    assert_eq!(env.events().all().events().len(), 0);
}
```

**Minimum negative-path categories to cover (where applicable):**

| Category | Example |
|----------|---------|
| Unauthorized caller | Wrong role or no role calls a restricted function |
| Invalid input | Zero amount, negative value, empty address |
| Illegal state | Double initialization, double pause, unpause when not paused |
| Paused contract | Any state-changing operation during pause |
| Asset lifecycle violation | Mint/transfer when asset is not Active |
| Cap/limit exceeded | Mint above supply cap, transfer above holding cap |
| Missing prerequisite | Accept admin transfer with no pending transfer |

---

## Integration & Fixture Tests

Changes that affect downstream consumers (SDKs, dashboards, indexers) require
integration-level coverage beyond unit tests.

### SDK Integration Fixtures

If your change alters any contract output that SDKs consume — event payloads,
error codes, restriction reasons, capability flags, or read-helper return
values — you must update the committed fixtures:

```bash
make update-fixtures
```

This regenerates `fixtures/sdk/` from live contract invocations. Review the
diff carefully and commit it alongside your logic change. The CI gate
(`make test-fixtures`) will fail if the committed fixtures drift from actual
behaviour.

See [`docs/sdk-fixtures.md`](sdk-fixtures.md) for the format specification.

### Manual Verification

Changes in certain categories require manual verification that automated tests
cannot fully cover:

| Change Category | Manual Verification Required |
|----------------|------------------------------|
| New deployment workflow | Deploy to local testnet, confirm `initialize` succeeds, verify events in the RPC response |
| New role or permission | Simulate every role combination against the new function, document in PR |
| Error code addition | Confirm the numeric value does not collide with existing codes in `docs/error-codes.md` |
| Event schema change | Confirm downstream SDKs can parse the new shape without breaking |
| State migration | Deploy before and after, verify storage keys are backward-compatible |
| Treasury/funds-affecting logic | Deploy to testnet, execute the full flow (mint → transfer → burn), verify balances match |

Document the manual verification steps and results in the PR description.

---

## No-Test Justification Guidance

Every PR must include tests. In rare cases, a change may qualify for a
no-test justification. If you believe your change needs no tests, you must
explicitly explain why in the PR description.

### When No-Test Justification Is Accepted

| Change Type | Justification |
|-------------|---------------|
| Documentation-only | No contract behaviour changed |
| Comment-only or doc-comment fix | No logic or state change |
| Renaming (non-functional) | No behaviour change — prove with `git diff --word-diff` showing only identifiers changed |
| Build/dependency configuration | No contract logic changed — but a CI-passing build must still be shown |
| CI workflow changes | No contract code changed — but the workflow must have run green |

### When No-Test Justification Is Rejected

| Change Type | Why Tests Are Required |
|-------------|----------------------|
| Bug fix | Without a regression test, the bug will reappear |
| New feature | Without tests, the feature is unproven |
| Refactor | Without tests, behaviour preservation (refactor safety) is unproven |
| Gas/performance optimization | Without benchmarks or invariant tests, correctness is unproven |
| Security fix | Without a test proving the exploit is closed, the fix is unverified |

### How to Submit a No-Test Justification

In the PR description, add a section:

```markdown
## No-Test Justification

- **Category:** Documentation-only
- **Reason:** No contract behaviour was changed. Only the README was updated
  to fix a broken link.
- **Evidence:** `git diff README.md` shows only the link URL changed.
```

A maintainer must explicitly approve the no-test justification before the PR
can merge.

---

## Running Tests

| Command | What It Does |
|---------|-------------|
| `make test` | Run all unit tests in `src/test.rs` |
| `make test-fixtures` | Verify committed SDK fixtures match live behaviour |
| `make update-fixtures` | Regenerate fixtures after an intentional change |
| `make verify` | fmt-check + clippy + test + build — run before pushing |

Always run `make verify` before opening or updating a PR. A CI failure caused
by missing or failing tests is grounds for immediate closure of the PR.
