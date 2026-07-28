# Asset Lifecycle

This document describes the five lifecycle states of the single asset managed
by this contract, the rules governing transitions between them, which
operations are permitted in each state, and how the asset lifecycle differs
from the existing contract-wide pause mechanism.

---

## States

| State     | Description |
|-----------|-------------|
| `Draft`   | Initial state. The asset has been registered but is not yet available for minting or transfers. No token operations are permitted. |
| `Active`  | Normal operating state. Minting and transfers are permitted (subject to all other compliance, cap, and whitelist checks). |
| `Paused`  | Asset operations are temporarily suspended at the lifecycle level — e.g. during a scheduled maintenance window or a pending governance action. Minting and transfers are blocked until the asset is reactivated. |
| `Retired` | Terminal state. The asset has been permanently decommissioned. Minting and transfers are permanently blocked. No further state transitions are possible. |
| `Blocked` | Asset operations are suspended pending review — e.g. a regulatory inquiry or a detected compliance issue. Minting and transfers are blocked. Unlike `Retired`, this state is recoverable: the admin can transition back to `Active` once the issue is resolved. |

---

## Transition diagram

```
 Draft ──────────────────────────────────────────▶ Active
                                                     │
                            ┌────────────────────────┤
                            │                        │
                            ▼                        ▼
                          Paused                  Blocked
                            │    ╲              ╱    │
                            │     ╲            ╱     │
                            │      ▼          ▼      │
                            │       Retired (▪)      │
                            │                        │
                            └──────────▶ Active ◀───┘
```

Valid transitions in table form:

| From      | To                          |
|-----------|-----------------------------|
| `Draft`   | `Active`                    |
| `Active`  | `Paused`, `Retired`, `Blocked` |
| `Paused`  | `Active`, `Retired`, `Blocked` |
| `Blocked` | `Active`                    |
| `Retired` | *(none — terminal)*         |

All other transitions (including setting the same state as the current one)
are rejected with `Error::InvalidLifecycleTransition`.

---

## Permitted operations per state

| Operation           | Draft | Active | Paused | Blocked | Retired |
|---------------------|:-----:|:------:|:------:|:-------:|:-------:|
| `mint_asset`        |  ✗    |  ✓     |  ✗     |  ✗      |  ✗      |
| `transfer`          |  ✗    |  ✓     |  ✗     |  ✗      |  ✗      |
| `distribute_yield`  |  ✓    |  ✓     |  ✓     |  ✓      |  ✓      |
| `get_asset_status`  |  ✓    |  ✓     |  ✓     |  ✓      |  ✓      |
| `set_asset_status`  |  ✓*   |  ✓*    |  ✓*    |  ✓*     |  ✗      |
| All read functions  |  ✓    |  ✓     |  ✓     |  ✓      |  ✓      |

*subject to transition rules above. Transitioning to `Active` is additionally
blocked when the contract-wide pause is active; all other target states are
settable regardless of the contract-wide pause state (see below).

### Error codes by rejected state

When `mint_asset` or `transfer` is called in a non-permitted lifecycle state
the contract reverts with a specific error so callers can distinguish the
root cause:

| Lifecycle state | Error returned           |
|-----------------|--------------------------|
| `Draft`         | `Error::AssetNotActive` (7000) |
| `Paused`        | `Error::AssetLifecyclePaused` (7001) |
| `Retired`       | `Error::AssetRetired` (7002) |
| `Blocked`       | `Error::AssetBlocked` (7003) |

---

## Authorization

`set_asset_status` is restricted to the **supreme admin** (the address stored
under `DataKey::Admin`). This is the same gating used for other
governance-level controls such as `propose_supply_cap`, `accept_supply_cap`,
and `unpause`. The `AssetManager` role is intentionally excluded: lifecycle
transitions are governance decisions, not day-to-day operational actions.

`set_asset_status` is **partially** restricted when the **contract-wide pause**
is active. Specifically, transitioning to `Active` is blocked while the
contract is paused, since reactivating an asset while the contract itself is
frozen is not operationally meaningful. All other target states — `Draft`,
`Paused`, `Blocked`, and `Retired` — are settable regardless of the
contract-wide pause state. This allows the admin to lock down or retire an
asset during an incident without a separate unpause step.

---

## Storage

The current lifecycle state is stored as a single `AssetStatus` value under
`DataKey::AssetStatus` in instance storage. When no value has been written
(i.e. on a freshly initialized contract before the first `set_asset_status`
call) it defaults to `AssetStatus::Draft`.

---

## Events

Every successful `set_asset_status` call emits an `AssetStatusChangedEvent`:

```rust
pub struct AssetStatusChangedEvent {
    pub admin: Address,
    pub previous_status: AssetStatus,
    pub new_status: AssetStatus,
}
```

Topic: `("asset_status_changed",)`

Failed calls (unauthorized, contract paused, invalid transition) revert
without emitting any event, consistent with the rest of the codebase.

---

## Distinction from the contract-wide pause

This contract has **two independent suspension mechanisms**. They are
evaluated separately and both must be clear for mint or transfer to proceed.

| | Contract-wide pause | Asset lifecycle state |
|---|---|---|
| **Where set** | `DataKey::Paused` (bool) | `DataKey::AssetStatus` (enum) |
| **Who can activate** | Admin or EmergencyOfficer (`pause`) | Supreme admin only (`set_asset_status`) |
| **Who can deactivate** | Supreme admin only (`unpause`) | Supreme admin only (`set_asset_status`) |
| **Scope** | Blocks *all* state-changing operations contract-wide (minting, transfers, whitelist changes, role changes, supply/holding cap changes) | Blocks only `mint_asset` and `transfer` |
| **Granularity** | Binary on/off | Five states with directional transition rules |
| **Recovery** | Always reversible via `unpause` | Reversible except from `Retired` (terminal) |
| **Typical use** | Emergency response, security incident, coordinated upgrade | Planned asset state management: pre-launch (Draft), regulatory hold (Blocked), wind-down (Retired) |

### Interaction

When both are active simultaneously for `mint_asset` and `transfer`,
`require_not_paused` is checked first (it appears first in those functions),
so `Error::ContractPaused` is returned rather than a lifecycle error. Once
the contract is unpaused the lifecycle check runs independently. This means:

- Asset `Active` + contract paused → `Error::ContractPaused`
- Asset `Paused` (lifecycle) + contract not paused → `Error::AssetLifecyclePaused`
- Asset `Paused` (lifecycle) + contract paused → `Error::ContractPaused`

For `set_asset_status` specifically, only transitions **to `Active`** are
blocked while the contract is paused. Transitions to `Paused`, `Blocked`,
`Retired`, or `Draft` are always permitted, allowing the admin to respond to
an incident (e.g. move the asset to `Blocked`) without needing a separate
unpause step first.

The contract-wide pause does **not** alter the stored `AssetStatus` value and
the asset lifecycle state does **not** alter the `Paused` flag. They are
orthogonal and must be managed independently.
