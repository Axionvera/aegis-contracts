use soroban_sdk::{contractimpl, contracttype, Address, Env};

use crate::admin::is_paused;
use crate::compliance::is_whitelisted;
use crate::holding::get_holding_cap;
use crate::lifecycle::{get_asset_status, AssetStatus};
use crate::{AegisContract, AegisContractArgs, AegisContractClient, DataKey};

// ─── Response types ─────────────────────────────────────────────────────────

/// Aggregated, read-only eligibility snapshot for a single investor address.
///
/// Composes the protocol's existing compliance whitelist, pause, holding-cap,
/// balance state, and asset lifecycle status into one response so SDK and
/// dashboard consumers do not need to stitch together several separate calls
/// — and risk reading them at inconsistent ledger states — to answer "can
/// this investor receive, hold, or send assets right now?"
/// See `docs/investor-eligibility.md`.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvestorEligibility {
    /// Whether the investor is on the compliance whitelist.
    pub whitelisted: bool,
    /// Whether the contract is currently paused. When `true`, no transfer or
    /// mint can succeed regardless of any other field on this struct.
    pub contract_paused: bool,
    /// The investor's current token balance.
    pub balance: i128,
    /// The currently active global per-investor holding cap. `0` means
    /// unrestricted (see `docs/investor-holding-restrictions.md`).
    pub holding_cap: i128,
    /// Remaining headroom under the holding cap (`holding_cap - balance`,
    /// floored at `0`). `None` when the holding cap is unrestricted (`0`).
    pub remaining_capacity: Option<i128>,
    /// The current lifecycle status of the asset. When not `Active`, no
    /// transfer or mint can succeed regardless of any other field.
    pub asset_status: AssetStatus,
    /// Whether this investor is currently eligible to receive a transfer or
    /// mint of at least `1` unit: whitelisted, contract not paused, asset
    /// lifecycle is Active, and (no holding cap or balance below the cap).
    pub can_receive: bool,
    /// Whether this investor is currently eligible to send a transfer of at
    /// least `1` unit: whitelisted, contract not paused, asset lifecycle is
    /// Active, and balance `> 0`.
    pub can_send: bool,
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Builds the eligibility snapshot for `investor`. Pure read — issues no
/// storage writes and never panics.
pub fn get_investor_eligibility(env: &Env, investor: &Address) -> InvestorEligibility {
    let whitelisted = is_whitelisted(env, investor);
    let contract_paused = is_paused(env);
    let asset_status = get_asset_status(env);
    let balance: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::Balance(investor.clone()))
        .unwrap_or(0);
    let holding_cap = get_holding_cap(env);

    let remaining_capacity = if holding_cap <= 0 {
        None
    } else {
        Some((holding_cap - balance).max(0))
    };
    let has_headroom = holding_cap <= 0 || balance < holding_cap;
    let asset_operable = asset_status == AssetStatus::Active;

    InvestorEligibility {
        whitelisted,
        contract_paused,
        balance,
        holding_cap,
        remaining_capacity,
        asset_status,
        can_receive: whitelisted && !contract_paused && asset_operable && has_headroom,
        can_send: whitelisted && !contract_paused && asset_operable && balance > 0,
    }
}

/// Returns whether a transfer of `amount` from `from` to `to` would currently
/// pass every check `transfer()` performs — pause state, compliance
/// whitelist for both parties, the receiver's holding cap, and the sender's
/// balance — evaluated against the current ledger state. Pure read — issues
/// no storage writes, requires no authorization, and never panics.
///
/// This is a point-in-time check only: balances, whitelist membership, the
/// holding cap, and pause state can all change between this call and a
/// subsequent `transfer` submission, so callers must still be prepared to
/// handle a revert.
pub fn check_transfer_eligibility(env: &Env, from: &Address, to: &Address, amount: i128) -> bool {
    if amount <= 0 {
        return false;
    }
    if is_paused(env) {
        return false;
    }
    if get_asset_status(env) != AssetStatus::Active {
        return false;
    }
    if !is_whitelisted(env, from) {
        return false;
    }
    if !is_whitelisted(env, to) {
        return false;
    }

    let from_balance: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::Balance(from.clone()))
        .unwrap_or(0);
    if from_balance < amount {
        return false;
    }

    let cap = get_holding_cap(env);
    if cap > 0 {
        let to_balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);
        if to_balance + amount > cap {
            return false;
        }
    }

    true
}

// ─── Public API ───────────────────────────────────────────────────────────────

#[contractimpl]
impl AegisContract {
    /// Returns an aggregated eligibility snapshot for `investor`: compliance
    /// (whitelist) status, pause state, holding-cap restriction, remaining
    /// capacity, current balance, and derived `can_send` / `can_receive`
    /// flags. Never mutates state and remains callable while paused. See
    /// `docs/investor-eligibility.md`.
    pub fn get_investor_eligibility(env: Env, investor: Address) -> InvestorEligibility {
        get_investor_eligibility(&env, &investor)
    }

    /// Returns whether a transfer of `amount` from `from` to `to` would pass
    /// all of the protocol's transfer-time checks as of the current ledger
    /// state (pause, compliance whitelist, holding cap, sender balance).
    /// Never mutates state and remains callable while paused. See
    /// `docs/investor-eligibility.md`.
    pub fn check_transfer_eligibility(env: Env, from: Address, to: Address, amount: i128) -> bool {
        check_transfer_eligibility(&env, &from, &to, amount)
    }
}
