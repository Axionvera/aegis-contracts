# Aegis Contracts Contribution Examples

What does a meaningful contribution look like — and what doesn't? This page
shows side-by-side comparisons of low-effort, partial, under-tested, failing-CI,
and acceptable contributions.

Use this as a **reference when preparing your PR** and as a **benchmark when
reviewing others' work**.

---

## Table of Contents

- [Low-Effort Examples](#low-effort-examples)
- [Partial Implementation Examples](#partial-implementation-examples)
- [Under-Tested Examples](#under-tested-examples)
- [Failing-CI Examples](#failing-ci-examples)
- [Acceptable Contribution Examples](#acceptable-contribution-examples)

---

## Low-Effort Examples

A low-effort contribution is one that compiles or reads plausibly but adds no
real contract behaviour, security enforcement, or testable correctness. It may
be a single comment, a formatting tweak, or a stub that never enforces protocol
rules.

### 1. Comment-only or whitespace change

```rust
// ❌ Adds a comment to an existing function. Zero behavioural change.
// This function mints asset tokens to a whitelisted investor after
// compliance checks. Only the asset manager may call it.
pub fn mint_asset(env: Env, admin: Address, to: Address, amount: i128) -> Result<(), Error> {
    require_role(&env, &admin, Role::AssetManager)?;
    require_not_paused(&env)?;
    // ... existing body unchanged from previous PR
}
```

**Why it's insufficient:** The function already existed and worked. The comment
adds nothing testable — no new behaviour, no security value, and no measurable
impact. Maintainers cannot evaluate this as meaningful work.

**What acceptable looks like:** A change that adds, modifies, or enforces
contract behaviour, with tests. See the
[Acceptable Contribution Examples](#acceptable-contribution-examples) section.

---

### 2. A stub function that does nothing

```rust
// ❌ Stub: declared but never enforces rules, writes nothing, emits nothing.
pub fn set_holding_cap(env: Env, admin: Address, investor: Address, cap: i128) -> Result<(), Error> {
    // TODO: implement holding cap storage and enforcement
    Ok(())
}
```

**Why it's insufficient:** The function compiles but does not write to storage,
does not emit an event, does not validate the cap, and does not enforce
authorization. It is dead code.

**What acceptable looks like:** See [`src/holding.rs`](../src/holding.rs) —
the real implementation calls `require_role`, validates `cap >= 0`, writes
`DataKey::HoldingCap` to persistence, and emits `holding_cap_updated`.

---

### 3. Formatting-only change across unrelated files

```rust
// ❌ Ran cargo fmt on files not touched by the issue. No behavioural change.
// diff shows only whitespace and reordered imports across 15 files.
```

**Why it's insufficient:** Formatting noise introduces merge conflicts for other
contributors and provides zero protocol value. Limit formatting to the files
your logic change touches.

**What acceptable looks like:** Include formatting only for the files you
actually modified. Run `make fmt-check` to confirm your changes are formatted,
but don't blanket-reformat the codebase.

---

## Partial Implementation Examples

A partial implementation satisfies some acceptance criteria but not all. It may
implement the happy path while skipping edge cases, security controls, or
failure modes.

### 1. Happy-path-only implementation

```rust
// ❌ Partial: implements success but omits validation, auth, and failure paths.
pub fn mint_to(env: Env, to: Address, amount: i128) -> Result<(), Error> {
    token::mint(&env, &to, &amount);
    env.events().publish(("mint",), (to, amount));
    Ok(())
}
```

**What's missing:**
- **Authorization** — anyone can mint. Needs `require_role(&env, &admin, Role::AssetManager)?`.
- **Pause check** — should call `require_not_paused(&env)?` before minting.
- **Input validation** — `amount` may be negative or zero. Should check `amount > 0`.
- **Compliance check** — recipient must be `Approved` in the compliance registry.
- **Tests** — only the happy path is tested; failure modes are missing entirely.

**What acceptable looks like:** A complete implementation in
[`src/asset.rs`](../src/asset.rs) enforces all five guards above and
includes tests for each rejection case. See the
[Meaningful Implementation Checklist](meaningful-implementation-checklist.md).

---

### 2. Missing event emissions

```rust
// ❌ Partial: writes state but doesn't emit the documented event.
pub fn revoke_investor(env: Env, admin: Address, investor: Address) -> Result<(), Error> {
    require_role(&env, &admin, Role::ComplianceOfficer)?;
    env.storage().instance().set(&DataKey::InvestorStatus(investor), &ComplianceStatus::Revoked);
    // ❌ No event emitted — indexers and dashboards won't see this change.
    Ok(())
}
```

**What's missing:**
- **Event emission** — off-chain systems rely on events to detect status changes.
  Must call `env.events().publish()` with the correct topic and payload as
  documented in [`docs/events.md`](events.md).
- **Transition matrix** — the
  [`docs/compliance-status-transitions.md`](compliance-status-transitions.md)
  state machine must be respected.

**What acceptable looks like:** See [`src/compliance.rs`](../src/compliance.rs) —
every state-changing operation emits its documented event, and tests assert
the event payload matches expectations.

---

### 3. Storage written without validation

```rust
// ❌ Partial: writes user input directly to storage without checking.
pub fn set_supply_cap(env: Env, admin: Address, new_cap: i128) -> Result<(), Error> {
    require_role(&env, &admin, Role::AssetManager)?;
    env.storage().instance().set(&DataKey::SupplyCap, &new_cap);
    // ❌ No validation: new_cap may be negative or below current supply.
    // ❌ No event emitted.
    Ok(())
}
```

**What's missing:**
- **Validation** — `new_cap` must be >= 0 and >= current total supply.
- **Event emission** — silent cap changes break off-chain monitoring.

**What acceptable looks like:** See [`src/supply_cap.rs`](../src/supply_cap.rs) —
the full 2-step governance workflow validates the cap, emits
`supply_cap_amended`, and includes failure-path tests for invalid values.

---

## Under-Tested Examples

An under-tested contribution adds real logic but lacks proof that it works.
Tests may exist but fail to cover failure paths, edge cases, or state
invariants.

### 1. Happy-path-only test

```rust
// ❌ Tests only the success case, never proves rejection of bad input.
#[test]
fn test_revoke_investor() {
    let e = setup();
    let result = revoke_investor(&e, admin, investor);
    assert!(result.is_ok());
    assert_eq!(compliance_status(&e, investor), ComplianceStatus::Revoked);
}
```

**What's missing:**
- **Unauthorized caller** — non-ComplianceOfficer should be rejected.
- **Double revoke** — revoking an already-revoked investor should succeed or
  be idempotent (as the spec defines).
- **Invalid address** — what happens with a zero-address investor?
- **Event assertion** — test does not verify the event was emitted with correct
  caller and payload.

**What acceptable looks like:** Tests in [`src/test.rs`](../src/test.rs) cover
every failure mode: unauthorized roles, paused contract, invalid inputs, and
idempotent calls. Each test asserts the returned error **and** the emitted
event.

---

### 2. Missing invariant tests

```rust
// ❌ Tests that the cap is set, but never that it is enforced on mint/transfer.
#[test]
fn test_set_holding_cap() {
    let e = setup();
    assert!(set_holding_cap(&e, admin, investor, 1000).is_ok());
}
```

**What's missing:**
- **Enforcement test** — does a transfer exceeding the holding cap actually
  fail with the correct error?
- **Cap removal** — does setting cap to 0 or `i128::MAX` effectively remove
  the restriction?
- **Cross-investor** — does investor A's cap affect investor B's transfers?

**What acceptable looks like:** See [`tests/sdk_fixtures.rs`](../tests/sdk_fixtures.rs)
for integration-style tests that exercise the full stack: set cap, attempt
transfer, assert rejection with the documented error code.

---

## Failing-CI Examples

A contribution that fails CI wastes reviewer time and blocks the pipeline.
These examples show common CI failures and how to prevent them.

### 1. Cargo fmt check failure

```diff
-    pub fn mint_asset(env:Env,admin:Address,to:Address,amount:i128)
+    pub fn mint_asset(env: Env, admin: Address, to: Address, amount: i128)
```

**Root cause:** Code was not formatted before pushing.

**Prevention:** Run `make fmt` before every commit, or configure your editor to
format on save. The CI gate runs `make fmt-check` and will reject unformatted
code.

**Fix:** `cargo fmt --all` and commit the formatting diff.

---

### 2. Clippy warning-as-error

```rust
// ❌ Causes: warning: unused variable: `admin`
pub fn mint(env: Env, admin: Address, to: Address, amount: i128) -> Result<(), Error> {
    require_not_paused(&env)?;
    // `admin` is never used — no authorization check.
    token::mint(&env, &to, &amount);
    Ok(())
}
```

**Root cause:** The `admin` parameter is unused because the authorization check
was omitted. Clippy treats unused variables as errors (`-D warnings`).

**Prevention:** Run `make clippy` locally before pushing. If a variable is
intentionally unused, prefix it with `_` (`_admin`).

**Fix:** Implement the missing `require_role(&env, &admin, ...)` call.

---

### 3. Test failure on edge case

```rust
// ❌ Assumes amount is always positive — fails when amount = 0.
pub fn mint_asset(env: Env, admin: Address, to: Address, amount: i128) -> Result<(), Error> {
    require_role(&env, &admin, Role::AssetManager)?;
    let total = current_supply(&env) + amount;  // ❌ No-op if amount = 0
    ...
}
```

**Root cause:** No input validation. Zero-amount minting may succeed
unexpectedly, breaking supply invariants.

**Prevention:** Write `amount > 0` checks in all minting and transfer paths.
Add a test that proves zero-amount calls are rejected.

**Fix:** Add `if amount <= 0 { return Err(Error::InvalidAmount); }` and
a corresponding test.

---

### 4. Build failure from missing WASM target

```text
error[E0463]: can't find crate for `core`
  |
  = note: the `wasm32v1-none` target may not be installed
```

**Root cause:** The `wasm32v1-none` Rust target is not installed. The CI build
step requires this target to compile the contract to WASM.

**Prevention:** Run `rustup target add wasm32v1-none` once after setting up the
repo. Run `make build` locally before pushing.

**Fix:** `rustup target add wasm32v1-none`

---

## Acceptable Contribution Examples

An acceptable contribution is complete: it changes contract behaviour in a
protocol-correct way, enforces security, emits events, includes tests for
happy and failure paths, passes CI, and satisfies all acceptance criteria.

### 1. Adding a holding cap enforcement path

**What it includes:**

```rust
// ✅ Enforces authorization, validates input, writes storage, emits event.
pub fn set_holding_cap(
    env: Env,
    admin: Address,
    investor: Address,
    cap: i128,
) -> Result<(), Error> {
    require_role(&env, &admin, Role::ComplianceOfficer)?;
    require_not_paused(&env)?;
    if cap < 0 {
        return Err(Error::InvalidAmount);
    }
    env.storage().instance().set(
        &DataKey::HoldingCap(investor.clone()),
        &cap,
    );
    env.events().publish(
        ("holding_cap_updated", "compliance"),
        (admin, investor, cap),
    );
    Ok(())
}
```

**Tests provided:**

```rust
#[test]
fn test_set_holding_cap_ok() { /* ... asserts event + storage */ }

#[test]
fn test_set_holding_cap_unauthorized() { /* ... asserts Error::NotAuthorized */ }

#[test]
fn test_set_holding_cap_when_paused() { /* ... asserts Error::ContractPaused */ }

#[test]
fn test_set_holding_cap_negative_cap() { /* ... asserts Error::InvalidAmount */ }
```

**Why it's acceptable:**
- Authorization enforced (`require_role`)
- Pause respected (`require_not_paused`)
- Input validated (`cap < 0` rejected)
- Storage written and event emitted
- Four tests cover success, unauthorized, paused, and invalid input
- CI passes: `make verify` is green

---

### 2. Fixing a compliance status transition bug

**What it includes:**

A PR that fixes an incorrect transition in the compliance state machine
(see [`docs/compliance-status-transitions.md`](compliance-status-transitions.md)).

**The fix:**
```rust
// ✅ Correctly prevents Approved -> Approved (no-op per spec).
fn transition_allowed(from: ComplianceStatus, to: ComplianceStatus) -> bool {
    matches!(
        (from, to),
        (ComplianceStatus::Pending, ComplianceStatus::Approved)
            | (ComplianceStatus::Approved, ComplianceStatus::Revoked)
            | (ComplianceStatus::Revoked, ComplianceStatus::Approved)
            | (_, ComplianceStatus::Blocked)
    )
}
```

**Evidence in the PR:**
- **Traceability table** maps the fix to the transition matrix in the docs.
- **Tests** added for every disallowed transition (12 combinations tested).
- **Event** emission verified for each allowed transition.
- **CI** is green — `make verify` passes.
- **Self-review** completed against the
  [Reviewer Checklist](reviewer-checklist.md).

**Why it's acceptable:**
- Behaviour is protocol-correct (matches the documented spec)
- All acceptance criteria from the issue are satisfied
- Security is maintained (no unauthorized transitions)
- Evidence is complete in the PR description

---

### 3. Adding a new read helper for dashboards

**What it includes:**

```rust
// ✅ Clean read helper with documented return format.
pub fn check_transfer_eligibility(
    env: Env,
    from: Address,
    to: Address,
    amount: i128,
) -> Result<TransferEligibility, Error> {
    let reasons = Vec::new(&env);
    if is_paused(&env) {
        reasons.push_back(TransferRestriction::AssetPaused);
    }
    if !is_compliant(&env, &from) {
        reasons.push_back(TransferRestriction::SenderNotCompliant);
    }
    if !is_compliant(&env, &to) {
        reasons.push_back(TransferRestriction::RecipientNotCompliant);
    }
    if amount > available_balance(&env, &from) {
        reasons.push_back(TransferRestriction::InsufficientBalance);
    }
    Ok(TransferEligibility { eligible: reasons.is_empty(), reasons })
}
```

**Why it's acceptable:**
- Returns structured data SDKs can consume (see
  [`docs/investor-eligibility.md`](investor-eligibility.md))
- Tests cover every restriction reason
- [SDK Integration Fixtures](sdk-fixtures.md) updated to include new outputs
- Documentation updated in [`docs/transfer-restrictions.md`](transfer-restrictions.md)
- All acceptance criteria met

---

## Summary

| Contribution Type | Signs | Verdict |
| :--- | :--- | :--- |
| **Low-effort** | Comment-only, stubs, formatting noise | Rejected — no behavioural change |
| **Partial** | Missing auth, validation, events, or tests | Needs rework — incomplete |
| **Under-tested** | Happy-path only, no failure tests | Needs tests — unproven |
| **Failing-CI** | Format, lint, build, or test failures | Blocked — fix locally first |
| **Acceptable** | Complete, secure, tested, documented | Ready for review |

**Before opening a PR:**
1. Run `make verify` — fmt-check + clippy + test + build must all pass
2. Complete the [Meaningful Implementation Checklist](meaningful-implementation-checklist.md)
3. Fill in the [Traceability Mapping](traceability-mapping.md) table in your PR
4. Self-review against the [Reviewer Checklist](reviewer-checklist.md)
5. Review these examples — is your PR in the last row?