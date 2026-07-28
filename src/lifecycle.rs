use soroban_sdk::{contractimpl, contracttype, panic_with_error, Address, Env};

use crate::admin::{get_admin, require_not_paused};
use crate::{AegisContract, AegisContractArgs, AegisContractClient, DataKey, Error};

// ─── State enum ───────────────────────────────────────────────────────────────

/// Lifecycle status for the single asset managed by this contract.
///
/// The state machine governs which operations (mint, transfer) are allowed at
/// any given time and which transitions the admin may trigger. The default
/// state is `Draft` (no status stored yet). `Retired` is the only terminal
/// state — once retired, the asset cannot be reactivated.
///
/// Valid transitions:
/// ```text
///  Draft ──────────────────────────────────▶ Active
///  Active ─────────────────────────────────▶ Paused
///  Active ─────────────────────────────────▶ Retired
///  Active ─────────────────────────────────▶ Blocked
///  Paused ─────────────────────────────────▶ Active
///  Paused ─────────────────────────────────▶ Retired
///  Paused ─────────────────────────────────▶ Blocked
///  Blocked ────────────────────────────────▶ Active
///  Retired ─  (terminal, no transitions out)
/// ```
///
/// Note: this "Paused" variant is entirely separate from the contract-level
/// `Paused` flag managed by `admin::pause()`/`unpause()`. Both checks are
/// evaluated independently in `mint_asset` and `transfer`.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetStatus {
    /// Asset has been created but not yet made available for minting or
    /// transfers. This is the initial default state.
    Draft,
    /// Asset is live. Minting and transfers are permitted (subject to all
    /// other compliance and cap checks).
    Active,
    /// Asset operations are suspended (lifecycle-level). Distinct from the
    /// contract-wide pause: both can be set simultaneously.
    Paused,
    /// Asset has been permanently retired. No further minting or transfers
    /// are possible. This state is terminal.
    Retired,
    /// Asset is blocked pending review (e.g. regulatory action). Minting and
    /// transfers are suspended until the admin explicitly unblocks the asset.
    Blocked,
}

// ─── Events ───────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct AssetStatusChangedEvent {
    pub admin: Address,
    pub previous_status: AssetStatus,
    pub new_status: AssetStatus,
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Returns the current lifecycle status of the asset. Defaults to `Draft`
/// if no status has been persisted yet (i.e. on a freshly initialized
/// contract before the first `set_asset_status` call).
pub fn get_asset_status(env: &Env) -> AssetStatus {
    env.storage()
        .instance()
        .get(&DataKey::AssetStatus)
        .unwrap_or(AssetStatus::Draft)
}

/// Asserts that the asset's lifecycle status permits minting and transfers
/// (i.e. the status is `Active`). Reverts with a specific error for each
/// non-operational status so callers can distinguish the root cause.
///
/// This check is **independent** of `require_not_paused` in `admin.rs` —
/// both must pass for mint/transfer to proceed.
pub fn require_asset_operable(env: &Env) {
    match get_asset_status(env) {
        AssetStatus::Active => {}
        AssetStatus::Draft => panic_with_error!(env, Error::AssetNotActive),
        AssetStatus::Paused => panic_with_error!(env, Error::AssetLifecyclePaused),
        AssetStatus::Retired => panic_with_error!(env, Error::AssetRetired),
        AssetStatus::Blocked => panic_with_error!(env, Error::AssetBlocked),
    }
}

/// Returns `true` if the given transition from `current` → `next` is allowed
/// by the lifecycle state machine rules.
fn is_valid_transition(current: &AssetStatus, next: &AssetStatus) -> bool {
    match (current, next) {
        // From Draft: can only activate.
        (AssetStatus::Draft, AssetStatus::Active) => true,
        // From Active: can pause, retire, or block.
        (AssetStatus::Active, AssetStatus::Paused) => true,
        (AssetStatus::Active, AssetStatus::Retired) => true,
        (AssetStatus::Active, AssetStatus::Blocked) => true,
        // From Paused: can reactivate, retire, or block.
        (AssetStatus::Paused, AssetStatus::Active) => true,
        (AssetStatus::Paused, AssetStatus::Retired) => true,
        (AssetStatus::Paused, AssetStatus::Blocked) => true,
        // From Blocked: can only reactivate.
        (AssetStatus::Blocked, AssetStatus::Active) => true,
        // Retired is terminal — no exits.
        // All other combinations are invalid.
        _ => false,
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

#[contractimpl]
impl AegisContract {
    /// Returns the current lifecycle status of the asset.
    ///
    /// Always callable — read operations are never blocked by lifecycle state
    /// or the contract-level pause.
    pub fn get_asset_status(env: Env) -> AssetStatus {
        get_asset_status(&env)
    }

    /// Transitions the asset to a new lifecycle state.
    ///
    /// Only the supreme admin can call this. Transitioning to `Active` is
    /// blocked while the contract is paused, since reactivating the asset
    /// while the contract itself is frozen is not operationally meaningful.
    /// All other target states (`Draft`, `Paused`, `Blocked`, `Retired`) are
    /// settable regardless of the contract-wide pause state, so the admin
    /// can lock down or retire an asset during an incident without a separate
    /// unpause step.
    ///
    /// The transition must follow the state machine rules (see `AssetStatus`
    /// docs). Setting the same status as the current one is rejected as a
    /// no-op. Transitioning out of `Retired` is always rejected.
    ///
    /// Emits `AssetStatusChangedEvent` on success.
    pub fn set_asset_status(
        env: Env,
        admin: Address,
        new_status: AssetStatus,
    ) -> Result<(), Error> {
        // Activating the asset while the contract is frozen is not allowed.
        // All other target states are permissible even while paused.
        if new_status == AssetStatus::Active {
            require_not_paused(&env);
        }
        admin.require_auth();
        if admin != get_admin(&env) {
            return Err(Error::Unauthorized);
        }

        let current = get_asset_status(&env);

        // Reject no-ops so event logs don't get polluted with spurious transitions.
        if current == new_status {
            return Err(Error::InvalidLifecycleTransition);
        }

        if !is_valid_transition(&current, &new_status) {
            return Err(Error::InvalidLifecycleTransition);
        }

        env.storage()
            .instance()
            .set(&DataKey::AssetStatus, &new_status);

        env.events().publish(
            ("asset_status_changed",),
            AssetStatusChangedEvent {
                admin,
                previous_status: current,
                new_status,
            },
        );

        Ok(())
    }
}
