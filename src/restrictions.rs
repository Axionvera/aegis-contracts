//! Transfer restriction reason codes.
//!
//! Every path that can block an asset movement (a `transfer` or a
//! `mint_asset`) resolves to exactly one [`RestrictionReason`]. The reason is
//! both:
//!
//! * **returned** by the read-only entrypoints in this module
//!   (`check_transfer_restriction` / `check_mint_restriction`), so a dashboard
//!   can explain *before* signing why a movement would be rejected; and
//! * **mirrored** by the numeric [`Error`] the state-changing entrypoint
//!   reverts with, so a client that only sees `Error(Contract, #<code>)` can
//!   recover the same explanation after the fact.
//!
//! The mapping between the two is total and lossless — see
//! [`error_for_reason`] and [`reason_for_error`], and
//! `docs/transfer-restrictions.md` for the SDK/dashboard mapping contract.
//!
//! This module deliberately contains **no** storage writes: the evaluators are
//! pure reads that never panic, so they stay callable while the contract is
//! paused, before `initialize`, and against terminal asset states.

use soroban_sdk::{contractimpl, contracttype, Address, Env};

use crate::admin::{get_role, is_paused};
use crate::asset::get_asset_status_internal;
use crate::compliance::is_whitelisted;
use crate::holding::get_holding_cap;
use crate::lifecycle::AssetStatus;
use crate::supply_cap::get_supply_cap;
use crate::{AegisContract, AegisContractArgs, AegisContractClient, DataKey, Error, Role};

// ─── Schema version ───────────────────────────────────────────────────────────

/// Schema version of the [`RestrictionReason`] enumeration.
///
/// Bump whenever a variant is **added**. Variants are append-only: never
/// remove, reorder, or repurpose one, and never renumber the numeric code a
/// variant maps to (same stability contract as `docs/error-codes.md`).
pub const RESTRICTION_SCHEMA_VERSION: u32 = 1;

// ─── Reason codes ─────────────────────────────────────────────────────────────

/// The single, specific reason an asset movement is (or would be) blocked.
///
/// Ordering is significant: [`evaluate_transfer`] and [`evaluate_mint`]
/// evaluate checks in exactly the order the corresponding state-changing
/// entrypoint does, so the reason returned by a pre-flight read is the same
/// reason the eventual revert reports. Clients must therefore treat the
/// response as "the first blocking reason", not "the only one".
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestrictionReason {
    /// No restriction applies — the movement passes every check as of the
    /// current ledger state. This is the only non-blocking variant.
    None = 0,

    // ── Authorization ──
    /// The caller does not hold the role required to perform the operation
    /// (e.g. minting without `AssetManager`). Mirrors [`Error::Unauthorized`].
    UnauthorizedOperation = 1,

    // ── Protocol / asset state ──
    /// The whole contract is under an emergency pause; nothing can move.
    ContractPaused = 2,
    /// The asset lifecycle status is `Paused` — temporary, may resume.
    AssetPaused = 3,
    /// The asset lifecycle status is `Retired` — terminal, never resumes.
    AssetRetired = 4,
    /// The asset lifecycle status is `Blocked` — administratively held.
    AssetBlocked = 5,

    // ── Compliance ──
    /// The sending address is not on the compliance whitelist.
    SenderNotCompliant = 6,
    /// The receiving address is not on the compliance whitelist.
    RecipientNotCompliant = 7,

    // ── Amount / balance / caps ──
    /// The amount is not strictly greater than zero.
    InvalidAmount = 8,
    /// The sender's balance cannot cover the amount.
    InsufficientBalance = 9,
    /// Crediting the recipient would breach the per-investor holding cap.
    HoldingCapExceeded = 10,
    /// The mint would breach the global supply cap.
    SupplyCapExceeded = 11,
}

impl RestrictionReason {
    /// Whether this reason actually blocks the movement.
    pub fn is_blocked(&self) -> bool {
        !matches!(self, RestrictionReason::None)
    }

    /// Whether the block is terminal for this asset: no retry, at any later
    /// ledger, under any state, will succeed. Only asset retirement is
    /// terminal — every other reason is at least theoretically recoverable
    /// (unpause, complete KYC, raise a cap, acquire balance). Dashboards
    /// should suppress "try again later" affordances for terminal reasons.
    pub fn is_terminal(&self) -> bool {
        matches!(self, RestrictionReason::AssetRetired)
    }
}

// ─── Reason ⇄ error mapping ───────────────────────────────────────────────────

/// The contract error a blocked movement reverts with for this reason.
///
/// Returns `None` only for [`RestrictionReason::None`], which does not block
/// and therefore has no corresponding error. This mapping is the normative
/// SDK contract documented in `docs/transfer-restrictions.md`.
pub fn error_for_reason(reason: &RestrictionReason) -> Option<Error> {
    match reason {
        RestrictionReason::None => None,
        RestrictionReason::UnauthorizedOperation => Some(Error::Unauthorized),
        RestrictionReason::ContractPaused => Some(Error::ContractPaused),
        RestrictionReason::AssetPaused => Some(Error::AssetPausedRestriction),
        RestrictionReason::AssetRetired => Some(Error::AssetRetiredRestriction),
        RestrictionReason::AssetBlocked => Some(Error::AssetBlockedRestriction),
        RestrictionReason::SenderNotCompliant => Some(Error::SenderNotWhitelisted),
        RestrictionReason::RecipientNotCompliant => Some(Error::ReceiverNotWhitelisted),
        RestrictionReason::InvalidAmount => Some(Error::InvalidAmount),
        RestrictionReason::InsufficientBalance => Some(Error::InsufficientBalance),
        RestrictionReason::HoldingCapExceeded => Some(Error::HoldingCapExceeded),
        RestrictionReason::SupplyCapExceeded => Some(Error::SupplyCapExceeded),
    }
}

/// The inverse of [`error_for_reason`]: recovers the restriction reason from
/// an observed contract error code.
///
/// Errors outside the movement-restriction surface (configuration, storage,
/// governance) return `None` — they are integration faults, not restrictions,
/// and must not be rendered as "your transfer was blocked because…".
pub fn reason_for_error(error: &Error) -> Option<RestrictionReason> {
    match error {
        Error::Unauthorized => Some(RestrictionReason::UnauthorizedOperation),
        Error::ContractPaused => Some(RestrictionReason::ContractPaused),
        Error::AssetPausedRestriction => Some(RestrictionReason::AssetPaused),
        Error::AssetRetiredRestriction => Some(RestrictionReason::AssetRetired),
        Error::AssetBlockedRestriction => Some(RestrictionReason::AssetBlocked),
        Error::SenderNotWhitelisted => Some(RestrictionReason::SenderNotCompliant),
        Error::ReceiverNotWhitelisted => Some(RestrictionReason::RecipientNotCompliant),
        Error::InvalidAmount => Some(RestrictionReason::InvalidAmount),
        Error::InsufficientBalance => Some(RestrictionReason::InsufficientBalance),
        Error::HoldingCapExceeded => Some(RestrictionReason::HoldingCapExceeded),
        Error::SupplyCapExceeded => Some(RestrictionReason::SupplyCapExceeded),
        _ => None,
    }
}

/// The numeric error code for a reason, or `0` for
/// [`RestrictionReason::None`]. Convenience for clients that key their
/// message catalogue off the raw `Error(Contract, #<code>)` integer.
pub fn code_for_reason(reason: &RestrictionReason) -> u32 {
    match error_for_reason(reason) {
        Some(err) => err as u32,
        None => 0,
    }
}

// ─── Shared check primitives ──────────────────────────────────────────────────

/// Maps the asset lifecycle status onto its restriction reason. `Active`
/// yields [`RestrictionReason::None`].
///
/// This is the check that turns the previously generic `AssetNotActive`
/// failure into three distinguishable outcomes — paused (retry later),
/// retired (never), blocked (contact the issuer).
pub fn asset_status_reason(status: &AssetStatus) -> RestrictionReason {
    match status {
        AssetStatus::Active => RestrictionReason::None,
        AssetStatus::Draft => RestrictionReason::AssetBlocked,
        AssetStatus::Paused => RestrictionReason::AssetPaused,
        AssetStatus::Retired => RestrictionReason::AssetRetired,
        AssetStatus::Blocked => RestrictionReason::AssetBlocked,
    }
}

fn balance_of(env: &Env, address: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Balance(address.clone()))
        .unwrap_or(0)
}

/// Whether `caller` may mint: the supreme admin or an `AssetManager`.
/// Reads only; returns `false` (rather than panicking) on an uninitialized
/// contract so the pre-flight reads stay panic-free.
fn can_mint(env: &Env, caller: &Address) -> bool {
    match env
        .storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
    {
        Some(admin) => *caller == admin || get_role(env, caller) == Role::AssetManager,
        None => false,
    }
}

// ─── Evaluators ───────────────────────────────────────────────────────────────

/// Resolves the first blocking reason for a transfer of `amount` from `from`
/// to `to`, or [`RestrictionReason::None`] if it would succeed.
///
/// Check order mirrors `transfer()` exactly: contract pause → amount → asset
/// lifecycle → sender compliance → recipient compliance → holding cap →
/// sender balance. Pure read: no writes, no auth, never panics.
///
/// Point-in-time only — state can change between this call and submission, so
/// callers must still handle a revert.
pub fn evaluate_transfer(
    env: &Env,
    from: &Address,
    to: &Address,
    amount: i128,
) -> RestrictionReason {
    if is_paused(env) {
        return RestrictionReason::ContractPaused;
    }
    if amount <= 0 {
        return RestrictionReason::InvalidAmount;
    }

    let status_reason = asset_status_reason(&get_asset_status_internal(env));
    if status_reason.is_blocked() {
        return status_reason;
    }

    if !is_whitelisted(env, from) {
        return RestrictionReason::SenderNotCompliant;
    }
    if !is_whitelisted(env, to) {
        return RestrictionReason::RecipientNotCompliant;
    }

    let cap = get_holding_cap(env);
    if cap > 0 && balance_of(env, to) + amount > cap {
        return RestrictionReason::HoldingCapExceeded;
    }

    if balance_of(env, from) < amount {
        return RestrictionReason::InsufficientBalance;
    }

    RestrictionReason::None
}

/// Resolves the first blocking reason for a mint of `amount` by `caller` to
/// `to`, or [`RestrictionReason::None`] if it would succeed.
///
/// Check order mirrors `mint_asset()`: contract pause → caller authorization →
/// amount → asset lifecycle → recipient compliance → supply cap → holding cap.
/// Pure read: no writes, no auth, never panics.
pub fn evaluate_mint(env: &Env, caller: &Address, to: &Address, amount: i128) -> RestrictionReason {
    if is_paused(env) {
        return RestrictionReason::ContractPaused;
    }
    if !can_mint(env, caller) {
        return RestrictionReason::UnauthorizedOperation;
    }
    if amount <= 0 {
        return RestrictionReason::InvalidAmount;
    }

    let status_reason = asset_status_reason(&get_asset_status_internal(env));
    if status_reason.is_blocked() {
        return status_reason;
    }

    if !is_whitelisted(env, to) {
        return RestrictionReason::RecipientNotCompliant;
    }

    let supply_cap = get_supply_cap(env);
    if supply_cap > 0 {
        let supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        if supply + amount > supply_cap {
            return RestrictionReason::SupplyCapExceeded;
        }
    }

    let holding_cap = get_holding_cap(env);
    if holding_cap > 0 && balance_of(env, to) + amount > holding_cap {
        return RestrictionReason::HoldingCapExceeded;
    }

    RestrictionReason::None
}

// ─── Public API ───────────────────────────────────────────────────────────────

#[contractimpl]
impl AegisContract {
    /// Returns the reason a transfer of `amount` from `from` to `to` would be
    /// blocked right now, or `RestrictionReason::None` if it would succeed.
    ///
    /// The returned reason is the same one the matching `transfer` revert
    /// reports via its numeric error code (see `get_restriction_code`). Never
    /// mutates state, requires no authorization, and remains callable while
    /// paused. See `docs/transfer-restrictions.md`.
    pub fn check_transfer_restriction(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
    ) -> RestrictionReason {
        evaluate_transfer(&env, &from, &to, amount)
    }

    /// Returns the reason a mint of `amount` by `caller` to `to` would be
    /// blocked right now, or `RestrictionReason::None` if it would succeed.
    /// Pure read; see `check_transfer_restriction`.
    pub fn check_mint_restriction(
        env: Env,
        caller: Address,
        to: Address,
        amount: i128,
    ) -> RestrictionReason {
        evaluate_mint(&env, &caller, &to, amount)
    }

    /// Returns the numeric contract error code a given restriction reason
    /// reverts with (`0` for `RestrictionReason::None`).
    ///
    /// Lets a client build its reason ⇄ code table from the deployment itself
    /// rather than hardcoding the mapping, so an SDK pinned to an older
    /// version cannot silently mis-label a code.
    pub fn get_restriction_code(_env: Env, reason: RestrictionReason) -> u32 {
        code_for_reason(&reason)
    }

    /// Returns the schema version of the restriction reason enumeration.
    pub fn get_restriction_schema_version(_env: Env) -> u32 {
        RESTRICTION_SCHEMA_VERSION
    }
}
