// The legacy `Events::publish((topic,), payload)` API is used intentionally:
// docs/events.md freezes these (topic, payload) shapes as a stable off-chain
// contract, and src/test.rs asserts them exactly. Migrating to the
// `#[contractevent]` macro must preserve every emitted shape byte-for-byte.
#![allow(deprecated)]
use soroban_sdk::{contractimpl, contracttype, Address, Env};

use crate::admin::{get_admin, require_not_paused};
use crate::{AegisContract, AegisContractArgs, AegisContractClient, DataKey};

// ─── Events ───────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct SupplyCapProposedEvent {
    pub admin: Address,
    pub current_cap: i128,
    pub proposed_cap: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct SupplyCapAmendedEvent {
    pub admin: Address,
    pub previous_cap: i128,
    pub new_cap: i128,
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Returns the currently active global supply cap.
///
/// A return value of `0` means "no cap enforced" — minting is unbounded
/// (still subject to the compliance whitelist). This is the safe default
/// before any cap has been proposed and accepted.
pub fn get_supply_cap(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::SupplyCap)
        .unwrap_or(0)
}

/// Returns the pending proposed cap, or `None` if no proposal is outstanding.
pub fn get_pending_supply_cap(env: &Env) -> Option<i128> {
    env.storage().instance().get(&DataKey::SupplyCapCandidate)
}

/// Enforces the active supply cap before a mint of `mint_amount`.
///
/// Reverts if a cap is set (`> 0`) and the resulting total supply would
/// exceed it. A cap of `0` is treated as "unbounded" and never blocks.
///
/// Note: this only constrains *future* minting. If the cap is later lowered
/// below the current total supply, existing supply is not burned; mints that
/// would push supply above the new cap simply fail until supply falls (via
/// burns/transfers out) or the cap is raised.
pub fn enforce_supply_cap(env: &Env, mint_amount: i128) {
    let cap = get_supply_cap(env);
    if cap <= 0 {
        return;
    }
    let supply: i128 = env
        .storage()
        .instance()
        .get(&DataKey::TotalSupply)
        .unwrap_or(0);
    assert!(
        supply + mint_amount <= cap,
        "Mint would exceed the active supply cap"
    );
}

// ─── Public API ───────────────────────────────────────────────────────────────

#[contractimpl]
impl AegisContract {
    /// Returns the currently active global supply cap (`0` = unbounded).
    pub fn get_supply_cap(env: Env) -> i128 {
        get_supply_cap(&env)
    }

    /// Returns the pending proposed supply cap, if any outstanding proposal
    /// exists. Returns `None` when there is nothing to accept.
    pub fn get_pending_supply_cap(env: Env) -> Option<i128> {
        get_pending_supply_cap(&env)
    }

    /// Proposes a new global supply cap using a 2-step governance flow.
    ///
    /// The proposal is recorded but NOT applied until `accept_supply_cap` is
    /// called by the admin. This prevents an accidental or malicious cap from
    /// taking effect immediately and bricking minting.
    ///
    /// Only the supreme admin can propose. Blocked when the contract is paused.
    /// A `proposed_cap` equal to the active cap is rejected as a no-op.
    pub fn propose_supply_cap(env: Env, admin: Address, proposed_cap: i128) {
        require_not_paused(&env);
        admin.require_auth();
        assert_eq!(
            admin,
            get_admin(&env),
            "Unauthorized: only admin can propose a supply cap"
        );
        assert!(proposed_cap >= 0, "Supply cap must be non-negative");

        let current = get_supply_cap(&env);
        assert_ne!(
            proposed_cap, current,
            "Proposed cap equals the active cap — no change requested"
        );

        env.storage()
            .instance()
            .set(&DataKey::SupplyCapCandidate, &proposed_cap);

        env.events().publish(
            ("supply_cap_proposed",),
            SupplyCapProposedEvent {
                admin,
                current_cap: current,
                proposed_cap,
            },
        );
    }

    /// Accepts and activates a previously proposed supply cap.
    ///
    /// Only the supreme admin can accept. Blocked when the contract is paused.
    /// Reverts if there is no outstanding proposal.
    pub fn accept_supply_cap(env: Env, admin: Address) {
        require_not_paused(&env);
        admin.require_auth();
        assert_eq!(
            admin,
            get_admin(&env),
            "Unauthorized: only admin can accept a supply cap"
        );

        let proposed: i128 = env
            .storage()
            .instance()
            .get(&DataKey::SupplyCapCandidate)
            .expect("No pending supply cap proposal to accept");

        let previous = get_supply_cap(&env);

        // Clear the proposal slot so it cannot be re-accepted.
        env.storage()
            .instance()
            .remove(&DataKey::SupplyCapCandidate);
        env.storage().instance().set(&DataKey::SupplyCap, &proposed);

        env.events().publish(
            ("supply_cap_amended",),
            SupplyCapAmendedEvent {
                admin,
                previous_cap: previous,
                new_cap: proposed,
            },
        );
    }

    /// Cancels an outstanding supply cap proposal without applying it.
    ///
    /// Only the supreme admin can cancel. Blocked when the contract is paused.
    /// Reverts if there is no outstanding proposal.
    pub fn cancel_supply_cap_proposal(env: Env, admin: Address) {
        require_not_paused(&env);
        admin.require_auth();
        assert_eq!(
            admin,
            get_admin(&env),
            "Unauthorized: only admin can cancel a supply cap proposal"
        );

        let had = env.storage().instance().has(&DataKey::SupplyCapCandidate);
        assert!(had, "No pending supply cap proposal to cancel");

        env.storage()
            .instance()
            .remove(&DataKey::SupplyCapCandidate);
    }
}
