use soroban_sdk::{contractimpl, contracttype, Address, Env};

use crate::admin::{get_admin, require_not_paused};
use crate::{AegisContract, AegisContractArgs, AegisContractClient, DataKey, Error};

// ─── Events ───────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct HoldingCapProposedEvent {
    pub admin: Address,
    pub current_cap: i128,
    pub proposed_cap: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct HoldingCapAmendedEvent {
    pub admin: Address,
    pub previous_cap: i128,
    pub new_cap: i128,
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Returns the currently active per-investor holding cap.
///
/// A return value of `0` means "no holding restriction" — any whitelisted
/// balance is permitted. This is the safe default before any cap has been
/// proposed and accepted.
pub fn get_holding_cap(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::HoldingCap)
        .unwrap_or(0)
}

/// Returns the pending proposed holding cap, or `None` if no proposal is
/// outstanding.
pub fn get_pending_holding_cap(env: &Env) -> Option<i128> {
    env.storage().instance().get(&DataKey::HoldingCapCandidate)
}

/// Enforces the per-investor holding cap after `address` is credited with
/// `incoming` tokens.
///
/// Returns `Err(Error::HoldingCapExceeded)` (code 7003) if a cap is set (`> 0`)
/// and the resulting balance would exceed it, so callers surface a specific
/// restriction reason instead of a generic host panic.
/// A cap of `0` is treated as "unrestricted" and never blocks. The check is
/// performed on the *resulting* balance so it applies uniformly to both
/// minting (crediting a new holder) and transfers (crediting a receiver).
///
/// Note: this constrains how much a single investor may hold. It does not
/// retroactively clamp existing balances; if the cap is lowered below a
/// holder's current balance, that holder simply cannot receive further
/// tokens until their balance falls (via transfers out) or the cap is raised.
pub fn enforce_holding_cap(env: &Env, address: &Address, incoming: i128) -> Result<(), Error> {
    let cap = get_holding_cap(env);
    if cap <= 0 {
        return Ok(());
    }
    let balance: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::Balance(address.clone()))
        .unwrap_or(0);
    if balance + incoming > cap {
        return Err(Error::HoldingCapExceeded);
    }
    Ok(())
}

// ─── Public API ───────────────────────────────────────────────────────────────

#[contractimpl]
impl AegisContract {
    /// Returns the currently active per-investor holding cap (`0` = unrestricted).
    pub fn get_holding_cap(env: Env) -> i128 {
        get_holding_cap(&env)
    }

    /// Returns the pending proposed holding cap, if any outstanding proposal
    /// exists. Returns `None` when there is nothing to accept.
    pub fn get_pending_holding_cap(env: Env) -> Option<i128> {
        get_pending_holding_cap(&env)
    }

    /// Proposes a new per-investor holding cap using a 2-step governance flow.
    ///
    /// The proposal is recorded but NOT applied until `accept_holding_cap` is
    /// called by the admin. This prevents an accidental or malicious cap from
    /// taking effect immediately and freezing investor balances.
    ///
    /// Only the supreme admin can propose. Blocked when the contract is paused.
    /// A `proposed_cap` equal to the active cap is rejected as a no-op.
    pub fn propose_holding_cap(env: Env, admin: Address, proposed_cap: i128) {
        require_not_paused(&env);
        admin.require_auth();
        assert_eq!(
            admin,
            get_admin(&env),
            "Unauthorized: only admin can propose a holding cap"
        );
        assert!(proposed_cap >= 0, "Holding cap must be non-negative");

        let current = get_holding_cap(&env);
        assert_ne!(
            proposed_cap, current,
            "Proposed cap equals the active cap — no change requested"
        );

        env.storage()
            .instance()
            .set(&DataKey::HoldingCapCandidate, &proposed_cap);

        env.events().publish(
            ("holding_cap_proposed",),
            HoldingCapProposedEvent {
                admin,
                current_cap: current,
                proposed_cap,
            },
        );
    }

    /// Accepts and activates a previously proposed holding cap.
    ///
    /// Only the supreme admin can accept. Blocked when the contract is paused.
    /// Reverts if there is no outstanding proposal.
    pub fn accept_holding_cap(env: Env, admin: Address) {
        require_not_paused(&env);
        admin.require_auth();
        assert_eq!(
            admin,
            get_admin(&env),
            "Unauthorized: only admin can accept a holding cap"
        );

        let proposed: i128 = env
            .storage()
            .instance()
            .get(&DataKey::HoldingCapCandidate)
            .expect("No pending holding cap proposal to accept");

        let previous = get_holding_cap(&env);

        env.storage()
            .instance()
            .remove(&DataKey::HoldingCapCandidate);
        env.storage()
            .instance()
            .set(&DataKey::HoldingCap, &proposed);

        env.events().publish(
            ("holding_cap_amended",),
            HoldingCapAmendedEvent {
                admin,
                previous_cap: previous,
                new_cap: proposed,
            },
        );
    }

    /// Cancels an outstanding holding cap proposal without applying it.
    ///
    /// Only the supreme admin can cancel. Blocked when the contract is paused.
    /// Reverts if there is no outstanding proposal.
    pub fn cancel_holding_cap_proposal(env: Env, admin: Address) {
        require_not_paused(&env);
        admin.require_auth();
        assert_eq!(
            admin,
            get_admin(&env),
            "Unauthorized: only admin can cancel a holding cap proposal"
        );

        let had = env.storage().instance().has(&DataKey::HoldingCapCandidate);
        assert!(had, "No pending holding cap proposal to cancel");

        env.storage()
            .instance()
            .remove(&DataKey::HoldingCapCandidate);
    }
}
