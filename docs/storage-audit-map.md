# Storage Audit Map

This document provides a complete audit map of all Soroban storage keys used by the Aegis RWA Protocol. It maps every key to its value type, storage class, owning functions, mutation points, invariants, failure paths, and linked test coverage.

## Storage Key Reference

### `Admin`

| Property | Value |
|---|---|
| **DataKey** | `DataKey::Admin` |
| **Value type** | `Address` |
| **Storage class** | Instance |
| **Lifecycle** | Set once at initialization; overwritten during admin transfer; removed on renounce |

| Function | Operation | Failure path | Test |
|---|---|---|---|
| `initialize` | **Write** (set) | Panics if already set ("Contract already initialized") | `test_double_initialization_reverts` |
| `get_admin` | **Read** | Panics if not set ("Admin not initialized") | (exercised by every auth test) |
| `accept_admin` | **Write** (set) | Panics if no pending candidate; panics if caller is not candidate | `test_full_admin_transfer` |
| `renounce_admin` | **Write** (remove) | Panics if caller is not admin | `test_renounce_admin_removes_admin` |

**Invariants**:
- Exactly one `Admin` exists after `initialize` and until `renounce_admin`.
- After `renounce_admin`, no `Admin` exists — all admin-gated operations permanently revert.
- After `accept_admin`, the old admin's `Admin` entry is overwritten; the new admin's entry is set.

---

### `AdminCandidate`

| Property | Value |
|---|---|
| **DataKey** | `DataKey::AdminCandidate` |
| **Value type** | `Address` |
| **Storage class** | Instance |
| **Lifecycle** | Set during 2-step transfer; cleared when transfer completes |

| Function | Operation | Failure path | Test |
|---|---|---|---|
| `transfer_admin` | **Write** (set) | Panics if caller is not admin | `test_transfer_admin_reverts_for_non_admin` |
| `accept_admin` | **Read** + **Write** (remove) | Panics if no pending transfer; panics if caller is not the candidate | `test_accept_admin_reverts_for_wrong_candidate`, `test_accept_admin_reverts_without_pending_transfer` |

**Invariants**:
- `AdminCandidate` is only set between `transfer_admin` and `accept_admin`.
- If `transfer_admin` is called again before `accept_admin`, the previous candidate is silently overwritten.
- `AdminCandidate` is always cleared after `accept_admin`.

---

### `Role(Address)`

| Property | Value |
|---|---|
| **DataKey** | `DataKey::Role(Address)` |
| **Value type** | `Role` (enum: `None`, `ComplianceOfficer`, `AssetManager`, `EmergencyOfficer`) |
| **Storage class** | Persistent |
| **Lifecycle** | Set at initialization (Admin); set/revoked via role management; overwritten during admin transfer |

| Function | Operation | Failure path | Test |
|---|---|---|---|
| `initialize` | **Write** (set to `Role::Admin`) | Panics if already initialized | `test_lifecycle` |
| `set_role` | **Write** | Panics if not admin; panics if assigning `Role::Admin` | `test_set_role_reverts_for_non_admin`, `test_cannot_assign_admin_role_via_set_role` |
| `remove_role` | **Write** (set to `Role::None`) | Panics if not admin; panics if target has no role | `test_remove_role_reverts_for_non_admin`, `test_remove_role_reverts_when_target_has_no_role` |
| `accept_admin` | **Write** (old admin → `None`, new admin → `Admin`) | Panics if no candidate; panics if wrong candidate | `test_full_admin_transfer` |
| `renounce_admin` | **Write** (set to `Role::None`) | Panics if not admin | `test_renounce_admin_removes_admin` |
| `get_role` | **Read** | Returns `Role::None` if not set | `test_get_role_returns_none_for_unassigned` |
| `require_role` | **Read** (via `get_role`) | Panics if caller lacks required role and is not admin | All RBAC tests |

**Invariants**:
- At most one address holds `Role::Admin` at any time.
- `Role::Admin` cannot be assigned via `set_role` — only via `initialize` or `accept_admin`.
- A `Role::None` entry in storage is functionally equivalent to no entry (both return `Role::None` on read).
- Persistent storage means role entries persist across contract instance upgrades (if any).

---

### `ComplianceStatus(Address)`

| Property | Value |
|---|---|
| **DataKey** | `DataKey::ComplianceStatus(Address)` |
| **Value type** | `ComplianceStatus` (enum: `Unknown`, `Pending`, `Approved`, `Revoked`, `Blocked`) |
| **Storage class** | Persistent |
| **Lifecycle** | Created on the address's first lifecycle transition; overwritten on every subsequent transition; never removed |

| Function | Operation | Failure path | Test |
|---|---|---|---|
| `set_compliance_status` | **Write** | `ContractPaused` if paused; `Unauthorized` if caller lacks a compliance role (or is not admin when leaving `Blocked`); `ComplianceStatusUnchanged` on a no-op; `InvalidComplianceTransition` if the matrix forbids it | `test_full_happy_path_lifecycle_walk`, `test_every_invalid_transition_is_rejected_exhaustively`, `test_only_admin_can_unblock`, `test_set_compliance_status_blocked_when_paused` |
| `batch_set_compliance_status` | **Write** | Same transition, authorization, and pause errors as `set_compliance_status`; rejects duplicate users with `InvalidComplianceTransition`; validates the whole batch before writing | `test_batch_set_compliance_status_updates_many_addresses_atomically`, `test_batch_set_compliance_status_rejects_invalid_entry_without_partial_write` |
| `whitelist_user` | **Write** (→ `Approved`) | As above; `InvalidComplianceTransition` if currently `Blocked` | `test_legacy_whitelist_wrappers_drive_the_lifecycle`, `test_legacy_whitelist_cannot_lift_a_block` |
| `revoke_whitelist` | **Write** (→ `Revoked`) | As above; a tolerated no-op from `Unknown` / `Blocked` | `test_legacy_revoke_does_not_downgrade_a_block`, `test_legacy_revoke_of_unknown_address_is_a_tolerated_no_op` |
| `get_compliance_status` | **Read** | Returns `Unknown` if not set; never panics | `test_compliance_status_defaults_to_unknown` |
| `require_can_send` / `require_can_receive` | **Read** | Returns a status-specific error code (4000/4002/4004 sender, 4001/4003/4005 receiver) | `test_mint_rejects_each_non_approved_status_with_its_own_code`, `test_transfer_rejects_each_non_approved_sender_status` |

**Invariants**:
- An address with no entry reads as `Unknown` — the fail-closed default that permits nothing.
- `Unknown` is never written as a target: compliance history is never erased.
- A transition is persisted only if `transition_is_allowed(current, new)` holds; rejected transitions leave storage untouched.
- Leaving `Blocked` requires the supreme admin and can only target `Pending`.
- Every persisted transition emits exactly one `compliance_status_changed` event carrying the previous and new state.
- Non-`Approved` states freeze a holder's `Balance` but never modify it.

See [Compliance Status Lifecycle](compliance-lifecycle.md) for the full matrix.

---

### `Whitelist(Address)`

| Property | Value |
|---|---|
| **DataKey** | `DataKey::Whitelist(Address)` |
| **Value type** | `bool` (stored as `true` when approved) |
| **Storage class** | Persistent |
| **Lifecycle** | **Derived mirror** of `ComplianceStatus(Address)`, kept for backwards compatibility. Written only by the lifecycle writer: set to `true` when a transition lands on `Approved`, removed on every other transition. |

| Function | Operation | Failure path | Test |
|---|---|---|---|
| `write_status` (internal) | **Write** (set/remove) | Never fails independently — always runs inside an already-validated transition | `test_full_happy_path_lifecycle_walk` |
| `is_whitelisted` (public) | **Read** | Derived from `ComplianceStatus`; returns `false` if not `Approved` | `test_read_functions_available_when_paused` |

**Invariants**:
- `Whitelist(addr) == true` ⟺ `ComplianceStatus(addr) == Approved`. The two keys are written together and cannot drift.
- **Not the source of truth.** Read `ComplianceStatus` instead; this key exists only so pre-lifecycle integrations keep working.
- A non-approved address has no `Whitelist(addr)` entry (reads as `false`).
- Status is not affected by pause state for reads — only writes are blocked.

---

### `Balance(Address)`

| Property | Value |
|---|---|
| **DataKey** | `DataKey::Balance(Address)` |
| **Value type** | `i128` |
| **Storage class** | Persistent |
| **Lifecycle** | Created on first mint; modified on transfer |

| Function | Operation | Failure path | Test |
|---|---|---|---|
| `mint_asset` | **Write** (read-modify-write: add `amount`) | Panics if paused; panics if receiver not whitelisted | `test_mint_succeeds_with_asset_manager_role` |
| `transfer` | **Write** (read-modify-write: subtract from `from`, add to `to`) | Panics if paused; panics if sender not whitelisted; panics if receiver not whitelisted; panics if insufficient balance | `test_lifecycle` |
| `get_balance_of` | **Read** | Returns `0` if not set | `test_read_functions_available_when_paused` |

**Invariants**:
- `Balance(addr)` is always `>= 0` (no negative balances).
- Sum of all `Balance` values equals `TotalSupply` (conservation of tokens).
- `Balance` is not set until the first `mint_asset` to that address.
- `transfer` atomically debits sender and credits receiver — no intermediate state where tokens are "in flight".
- Persistent storage means balances survive contract instance upgrades.

**Conservation check** (off-chain):
```
sum(Balance[addr] for all addr) == TotalSupply
```

---

### `TotalSupply`

| Property | Value |
|---|---|
| **DataKey** | `DataKey::TotalSupply` |
| **Value type** | `i128` |
| **Storage class** | Instance |
| **Lifecycle** | Incremented on mint; never decremented |

| Function | Operation | Failure path | Test |
|---|---|---|---|
| `mint_asset` | **Write** (read-modify-write: add `amount`) | Panics if paused; panics if receiver not whitelisted | `test_mint_succeeds_with_asset_manager_role` |
| `get_total_supply` | **Read** | Returns `0` if not set | `test_read_functions_available_when_paused` |

**Invariants**:
- `TotalSupply` is monotonically increasing (only incremented, never decremented).
- `TotalSupply` >= 0.
- There is no burn function — `TotalSupply` can only increase.
- `distribute_yield` does not modify `TotalSupply` (it is a mock/placeholder).
- Instance storage means `TotalSupply` is bound to the contract instance lifecycle.

---

### `Paused`

| Property | Value |
|---|---|
| **DataKey** | `DataKey::Paused` |
| **Value type** | `bool` |
| **Storage class** | Instance |
| **Lifecycle** | Set to `true` on pause; set to `false` on unpause |

| Function | Operation | Failure path | Test |
|---|---|---|---|
| `pause` | **Write** (set to `true`) | Panics if not admin/EmergencyOfficer; panics if already paused | `test_pause_succeeds_for_admin`, `test_pause_reverts_when_already_paused` |
| `unpause` | **Write** (set to `false`) | Panics if not admin; panics if not paused | `test_unpause_succeeds_for_admin`, `test_unpause_reverts_when_not_paused` |
| `is_paused` | **Read** | Returns `false` if not set (default) | `test_read_functions_available_when_paused` |

**Invariants**:
- `Paused` defaults to `false` (not set) — the contract starts unpaused.
- When `true`, all state-changing operations revert except `pause` and `unpause`.
- Read functions (`get_role_of`, `get_balance_of`, `get_total_supply`, `is_whitelisted`) remain available regardless of pause state.
- Only Admin can unpause — EmergencyOfficer cannot.

---

## Cross-Key Invariants

### Token Conservation
```
∀ addr: Balance(addr) >= 0
∑ Balance(addr) == TotalSupply
```
**Test coverage**: `test_lifecycle`, `test_pause_unpause_full_lifecycle`

### Single Admin
```
∃! addr: Role(addr) == Role::Admin
```
After `initialize`, exactly one address holds the Admin role. This is maintained through `accept_admin` (atomic swap) and broken only by `renounce_admin`.
**Test coverage**: `test_full_admin_transfer`, `test_renounce_admin_removes_admin`

### Compliance Lifecycle Gating
```
∀ mint_asset(to):        ComplianceStatus(to)   == Approved
∀ transfer(from, to):    ComplianceStatus(from) == Approved ∧ ComplianceStatus(to) == Approved
∀ transition(from, to):  transition_is_allowed(from, to)
```
Tokens can only be created at or transferred between compliance-`Approved`
addresses; every other lifecycle state fails closed with its own error code.
Status changes themselves are constrained by the transition matrix.
**Test coverage**: `test_mint_rejects_each_non_approved_status_with_its_own_code`, `test_transfer_rejects_each_non_approved_sender_status`, `test_transfer_rejects_each_non_approved_receiver_status`, `test_every_invalid_transition_is_rejected_exhaustively`, `test_lifecycle`

### Pause Immutability
```
Paused == true ⟹ ∀ op ∈ {mint, transfer, set_compliance_status, whitelist, revoke, yield, set_role, remove_role, transfer_admin, accept_admin, renounce_admin}: op() reverts
```
**Test coverage**: `test_mint_blocked_when_paused`, `test_transfer_blocked_when_paused`, `test_whitelist_blocked_when_paused`, `test_revoke_whitelist_blocked_when_paused`, `test_distribute_yield_blocked_when_paused`, `test_set_role_blocked_when_paused`, `test_remove_role_blocked_when_paused`

### RBAC Enforcement
```
∀ privileged_op(caller, ...): caller == Admin ∨ Role(caller) == required_role
```
**Test coverage**: All "wrong-caller" tests in `test.rs`

---

## Failure-Path State Expectations

| Failure | State after revert | Storage affected |
|---|---|---|
| `initialize` double-call | No change | None |
| `mint_asset` paused | No change | None |
| `mint_asset` receiver not whitelisted | No change | None |
| `mint_asset` unauthorized | No change | None |
| `transfer` paused | No change | None |
| `transfer` insufficient balance | No change | None |
| `transfer` sender not whitelisted | No change | None |
| `transfer` receiver not whitelisted | No change | None |
| `whitelist_user` paused | No change | None |
| `whitelist_user` unauthorized | No change | None |
| `revoke_whitelist` paused | No change | None |
| `revoke_whitelist` unauthorized | No change | None |
| `set_role` paused | No change | None |
| `set_role` assigning Admin | No change | None |
| `set_role` unauthorized | No change | None |
| `remove_role` no role to revoke | No change | None |
| `remove_role` unauthorized | No change | None |
| `transfer_admin` unauthorized | No change | None |
| `accept_admin` wrong candidate | No change | None |
| `accept_admin` no pending transfer | No change | None |
| `renounce_admin` unauthorized | No change | None |
| `pause` already paused | No change | None |
| `unpause` not paused | No change | None |
| `unpause` unauthorized | No change | None |

All failing operations are atomic — Soroban reverts the entire transaction on panic, leaving storage untouched.

---

## Soroban Storage Type Implications

| Type | Behaviour | Used for |
|---|---|---|
| **Instance** | Bound to contract instance lifecycle. Cleared if contract is re-installed. | Admin, AdminCandidate, TotalSupply, Paused |
| **Persistent** | Survives across contract upgrades. Requires rent exemption. | Role, Whitelist, Balance |

**Risk**: If the contract instance is replaced (not upgraded in-place), all Instance storage is lost. This would:
- Remove the `Admin` (contract becomes ungovernable).
- Reset `TotalSupply` to 0 (but balances persist — supply would be inconsistent).
- Reset `Paused` to `false` (contract resumes in unpaused state).

**Mitigation**: Use Soroban's built-in contract upgrade mechanism (which preserves instance storage) rather than deploying a new contract.
