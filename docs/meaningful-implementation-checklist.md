# Meaningful Contract Implementation Checklist

What counts as *meaningful* Aegis contract work — and what doesn't. Use this
before opening a PR and before reviewing one. The goal is protocol-correct,
secure, tested behaviour, not just lines of code.

## What "meaningful" means

A contribution is meaningful when it changes **contract behaviour** in a way
that is:

1. **Protocol-correct** — matches the documented spec and compliance model.
2. **Secure by construction** — enforces authorization, bounds, and invariants.
3. **Observable** — emits the right events and returns stable error codes.
4. **Verified** — covered by tests that prove the behaviour, including failures.
5. **Acceptance-driven** — satisfies the issue's stated criteria, not a subset.

A small PR can be meaningful (a tight, well-tested fix). A large PR can be
*un*meaningful (adds code without behaviour, tests, or security). Size is not
the measure — **completeness is**.

---

## Before you start

- [ ] Read the relevant spec doc: [`docs/contract-spec.md`](contract-spec.md),
      [`docs/admin-roles.md`](admin-roles.md), [`docs/compliance-registry-reads.md`](compliance-registry-reads.md),
      [`docs/investor-eligibility.md`](investor-eligibility.md), [`docs/investor-holding-restrictions.md`](investor-holding-restrictions.md).
- [ ] Identify which contract module owns the behaviour (e.g. `asset.rs`, `compliance.rs`,
      `admin.rs`, `holding.rs`, `eligibility.rs`, `supply_cap.rs`).
- [ ] Note the authorization model: who may call this? (Admin / scoped role /
      public). See [`docs/admin-roles.md`](admin-roles.md) and
      [`src/admin.rs`](../src/admin.rs) (`require_role`, `require_any_role`).
- [ ] Note the relevant events ([`docs/events.md`](events.md)) and error codes
      ([`docs/error-codes.md`](error-codes.md)).

---

## Implementation standards

### Protocol behaviour
- [ ] The change implements the **actual rule**, not a stub or comment.
- [ ] Authorization is enforced at the entry point (e.g. `require_role`,
      `require_not_paused`) — not assumed.
- [ ] Numeric invariants hold: non-negative amounts, caps respected
      (`enforce_holding_cap`, supply cap), no overflow/underflow paths.
- [ ] Compliance is enforced where the spec requires it (whitelist checks in
      `transfer`/`mint`, eligibility checks in `check_transfer_eligibility`).

### Security impact
- [ ] No privilege escalation: a scoped role cannot perform admin-only actions.
- [ ] No unauthorized state mutation (storage writes gated by auth checks).
- [ ] Pause / emergency controls respected where applicable
      ([`docs/emergency-pause.md`](emergency-pause.md), `require_not_paused`).
- [ ] Input validation rejects invalid values early (negative amounts, zero
      addresses, out-of-range enums).
- [ ] Cross-referenced with [`docs/threat-model.md`](threat-model.md) and
      [`docs/admin-misuse-risks.md`](admin-misuse-risks.md) for the affected area.

### Events
- [ ] State-changing actions emit the documented event topic with the correct
      payload struct ([`docs/events.md`](events.md)).
- [ ] Event `caller` is the authorizing address, not a hardcoded admin.
- [ ] No silent state changes without an event (off-chain systems rely on them).

### Errors
- [ ] Failures revert with a **stable `Error` variant**
      ([`docs/error-codes.md`](error-codes.md), [`src/errors.rs`](../src/errors.rs)),
      never a bare `panic!`/`assert!` string.
- [ ] Error codes are matched by SDKs on the numeric value, not on text.

### Tests
- [ ] New behaviour has tests in the module's `test.rs` / inline `#[cfg(test)]`.
- [ ] **Failure paths are tested**, not just the happy path
      (unauthorized caller, paused contract, cap exceeded, invalid input).
- [ ] Tests assert both the returned value **and** the emitted event / error
      where relevant.
- [ ] Local `make test` is green (see
      [`docs/failing-ci-guide.md`](failing-ci-guide.md)).

### Acceptance criteria
- [ ] Every checkbox in the issue is satisfied — or explicitly noted as
      out-of-scope with rationale.
- [ ] Docs/README updated if the change affects external interfaces
      (events, errors, roles, eligibility).

---

## Examples of *insufficient* (small/incomplete) work

> These illustrate what NOT to submit. They look like progress but fail the
> standards above.

**1. A stub with no behaviour**
```rust
// ❌ Does not enforce anything; no auth, no cap, no event.
pub fn set_holding_cap(env: Env, admin: Address, cap: i128) {
    // TODO: implement
}
```
Meaningful version enforces `require_role`, validates `cap >= 0`, writes
storage, and emits the `holding_cap_updated` event.

**2. Behaviour without authorization**
```rust
// ❌ Anyone can mint — privilege escalation.
pub fn mint_asset(env: Env, to: Address, amount: i128) -> Result<(), Error> {
    token::mint(&env, &to, &amount);
    Ok(())
}
```
Meaningful version takes an `admin` caller and calls `require_role(env, &admin, Role::AssetManager)`
before minting, and emits `asset_minted`.

**3. Happy-path-only tests**
```rust
// ❌ Only proves the success case; never proves rejection of bad input.
#[test]
fn test_mint_ok() {
    let e = ...; mint_asset(&e, user, 100); assert_eq!(balance(&e, user), 100);
}
```
Meaningful version also asserts: non-admin is rejected, `amount <= 0` is
rejected, and the `asset_minted` event is emitted with correct `caller`.

**4. Panic instead of stable error**
```rust
// ❌ Off-chain SDKs cannot match on this; not in the error-code standard.
assert!(cap >= 0, "cap must be non-negative");
```
Meaningful version returns `Err(Error::InvalidAmount)` so SDKs map it via
[`docs/error-codes.md`](error-codes.md).

**5. Silent state change (no event)**
```rust
// ❌ State changed but no event emitted — indexers miss it.
env.storage().instance().set(&KEY, &new_value);
```
Meaningful version also calls `env.events().publish(("topic",), payload)`.

---

## Reviewer checks (use this on every PR)

- [ ] Does the PR change behaviour, or just add code/comments?
- [ ] Is authorization enforced at the entry point for every mutating call?
- [ ] Are numeric invariants (caps, non-negative) actually enforced?
- [ ] Does it emit the correct documented events?
- [ ] Does it use stable `Error` codes, not panic strings?
- [ ] Are both happy and failure paths tested?
- [ ] Does `make build` + `make test` pass locally? (Failing checks block approval.)
- [ ] Are all issue acceptance criteria met (or documented as out-of-scope)?
- [ ] Any security concern vs [`docs/threat-model.md`](threat-model.md)?

If the answer to the first question is "just code/comments," the PR is not
meaningful yet — request the missing behaviour, tests, or security enforcement
before approval.
