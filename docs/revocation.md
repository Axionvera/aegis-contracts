# Compliance Revocation Lifecycle

## Overview

The protocol must handle investors who are no longer compliant after having been previously whitelisted. This document defines the revocation lifecycle, state, admin paths, guards, events, and off-chain implications.

## Motivation

Regulatory requirements (KYC/AML, sanctions screening) can change after onboarding. An investor may:
- Fail ongoing KYC refresh
- Appear on sanctions list
- Violate transfer restrictions
- Request off-boarding

The contract needs a deterministic, auditable way to suspend such users.

## State Model

Two persistent flags per address stored in `persistent()` storage:

| Key | Type | Meaning |
|-----|------|---------|
| `Whitelist(Address)` | `bool` | KYC approved |
| `Revoked(Address)` | `bool` | Compliance revoked / frozen |

Effective compliance check:

```rust
fn is_whitelisted(env, user) -> bool {
  Whitelist[user] == true && Revoked[user] == false
}
fn is_revoked(env, user) -> bool {
  Revoked[user] == true
}
```

- Never whitelisted: both false
- Whitelisted: `Whitelist=true, Revoked=false`
- Revoked: `Whitelist=false, Revoked=true`
- Re-whitelisted: back to `true/false`
- Unrevoked (intermediate): `Whitelist=false, Revoked=false` -> still blocked until whitelist

## Admin Update Path

Only admin (`instance().Admin`) can call:

- `whitelist_user(admin, user)` : `require_auth`, sets `Whitelist=true`, `Revoked=false`, emits `("aegis","wl_add",user)`.
- `revoke_user(admin, user)` : `require_auth`, sets `Whitelist=false`, `Revoked=true`, emits `("aegis","wl_rev",user)`.
- `unrevoke_user(admin, user)` : Clears revoked flag only, useful for two-step governance (clear sanction then re-KYC). Recommend using `whitelist_user` for single-step re-onboarding.
- View helpers: `is_whitelisted_check`, `is_revoked_check`, `compliance_status` -> return tuple `(bool,bool)`.

All admin calls are synchronous and emit events for off-chain indexing.

## Transfer and Minting Guards

### Minting

```rust
assert!(!is_revoked(&env, &to), "Receiver is revoked");
assert!(is_whitelisted(&env, &to), "Receiver is not whitelisted");
```

- Revoked recipients **cannot receive** new restricted tokens.
- Error messages distinguish revocation vs never-whitelisted.

### Transfer

```rust
assert!(!is_revoked(&env, &from), "Sender is revoked");
assert!(!is_revoked(&env, &to), "Receiver is revoked");
assert!(is_whitelisted(&env, &from), "Sender is not whitelisted");
assert!(is_whitelisted(&env, &to), "Receiver is not whitelisted");
```

#### Policy: Fully Blocked / Frozen

Revoked users are fully frozen:
- Cannot receive mint
- Cannot receive transfer-in
- Cannot send transfer-out
- Can hold existing balance (not burned) - retained for audit, yield snapshot, forced redemption

**Rationale**: A sanctioned address must not be able to move tokens without admin oversight. Allowing exit-only (`transfer-out` allowed) would let a revoked user offload to a secondary market without control. Fully frozen forces off-boarding via legal process or via admin-mediated redemption. If governance decides exit-only is required, change is single line: remove `Sender is revoked` check.

## Events

| Event struct | Topics | Data | When |
|--------------|--------|------|------|
| `WhitelistAdd` | `("aegis","wl_add",user)` | `admin` | Whitelist or re-whitelist |
| `WhitelistRevoked` | `("aegis","wl_rev",user)` | `admin` | Revocation |

Both use `#[contractevent]` so they appear in contract spec XDR. Monitoring can subscribe to whole namespace `aegis` and filter topic1=`wl_rev` for compliance alerts.

### Off-chain handling

On `wl_rev`:
- Mark address frozen in DB
- Block UI investment
- Alert risk desk via pattern alert (e.g., Slack, PagerDuty)
- Initiate forced redemption workflow if needed
- Log audit

On subsequent `wl_add`:
- Clear frozen flag
- Log re-onboarding

## Tests

Coverage in `src/test.rs`:

- `test_revoke_emits_event`: revocation emits `wl_rev` event with correct topics/data
- `test_revoked_cannot_receive_mint`: mint to revoked panics "Receiver is revoked"
- `test_revoked_cannot_receive_transfer`: transfer to revoked panics "Receiver is revoked"
- `test_revoked_cannot_send_transfer_fully_blocked`: transfer from revoked panics "Sender is revoked" - validates fully blocked policy
- `test_revoked_retains_balance_but_frozen`: balance not burned, compliance_status returns `(false,true)`
- `test_rewhitelist_clears_revocation`: whitelist after revoke restores ability to mint/transfer
- `test_revocation_lifecycle_observable`: init + whitelist + mint + revoke emits 4 namespaced events in order, including `wl_rev`
- `test_non_whitelisted_still_blocked_after_unrevoke_without_whitelist`: unrevoke without whitelist still blocks mint (whitelist false)

All 17 tests pass (1 ignored XDR dump).

## Future Extensions

- Batch revocation: `revoke_users(admin, Vec<Address>)` to save gas
- Reason code: store `RevocationReason` enum or string for audit
- Timed suspension: `RevokedUntil` ledger timestamp for temporary suspension
- Forced redemption: admin burns frozen balance and triggers off-chain payout
- Allow-list bypass for redemption contract: escrow contract whitelisted to receive from revoked users during off-boarding

## Confidence

Implementation confidence >95%: 
- State implemented with persistent storage, no archival risk
- Guards cover mint and transfer both sides
- Events emitted and tested
- Policy documented as fully frozen with rationale
- WASM builds and all tests green
