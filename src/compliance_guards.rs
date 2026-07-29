//! Compliance status transition guards.
//!
//! The compliance lifecycle in [`crate::compliance`] answers *what* the legal
//! states and edges are. This module answers the operational question that
//! sits on top of it: **"would this specific caller's status change succeed
//! right now, and if not, exactly why?"**
//!
//! Every precondition a committed transition must satisfy — initialization,
//! the global pause, the caller's authority (including the admin-only exit
//! from `Blocked`), no-op rejection, and the transition matrix itself — is
//! evaluated here, in one ordered pass, by a function that **never panics and
//! never writes**. Two consumers share that single evaluation:
//!
//! * **Enforcement.** `set_compliance_status`, `batch_set_compliance_status`,
//!   `whitelist_user`, and `revoke_whitelist` all reach their verdict through
//!   [`evaluate_transition`]. There is no second copy of the rules that could
//!   drift from the one clients can read.
//! * **Pre-flight.** `check_compliance_transition` /
//!   `check_compliance_batch` return the same verdict as a typed
//!   [`ComplianceTransitionCheck`], so a dashboard can disable an illegal
//!   action and explain it *before* an officer signs a transaction, instead of
//!   submitting one and translating a revert.
//!
//! Because both paths are the same code, a pre-flight `allowed == true` is a
//! statement about the ledger state at read time — not a reservation. State
//! can change between the read and the submission (a pause, a role
//! revocation, another officer's write), so callers must still handle a
//! revert. See `docs/compliance-transition-guards.md`.

use soroban_sdk::{contractimpl, contracttype, vec, Address, Env, Vec};

use crate::admin::{get_role, is_paused};
use crate::compliance::{
    get_compliance_status, transition_is_allowed, ComplianceBatchUpdate, ComplianceStatus,
};
use crate::{AegisContract, AegisContractArgs, AegisContractClient, DataKey, Error, Role};

// ─── Guard reasons ────────────────────────────────────────────────────────────

/// Why a compliance status transition is permitted or rejected.
///
/// Exactly one reason is returned per evaluation: the **first** precondition
/// that fails, in the same order enforcement applies them. A client that wants
/// to surface every problem with a proposed change must fix one and re-check.
///
/// The variant order is part of the contract's ABI: variants are append-only
/// and must never be reordered or repurposed (same stability contract as the
/// `docs/error-codes.md` numeric codes and the `docs/events.md` topics).
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionGuard {
    /// Every precondition passes: this caller may commit this transition
    /// against the current ledger state.
    Allowed,
    /// The contract has not been initialized, so no authority exists to check
    /// the caller against. Maps to `NotInitialized` (2000).
    NotInitialized,
    /// The contract is globally paused. Every compliance write is blocked
    /// while the pause is in force, regardless of caller or status. Maps to
    /// `ContractPaused` (3004). Recoverable: the admin can `unpause`.
    ContractPaused,
    /// The caller holds neither the ComplianceOfficer nor the
    /// EmergencyOfficer role and is not the admin. Maps to `Unauthorized`
    /// (3000).
    CallerUnauthorized,
    /// The address is currently `Blocked` and the caller is not the supreme
    /// admin. Distinguished from [`Self::CallerUnauthorized`] because the
    /// caller may hold a perfectly valid compliance role and still be refused
    /// here: lifting a sanctions freeze is admin-only by design. Maps to
    /// `Unauthorized` (3000) — the same code, a different remediation
    /// ("escalate to the admin", not "request a role").
    BlockedRequiresAdmin,
    /// The requested status equals the current status. Rejected so a no-op
    /// can never emit a misleading lifecycle event. Maps to
    /// `ComplianceStatusUnchanged` (4007).
    StatusUnchanged,
    /// The requested target is `Unknown`. Compliance history is never erased;
    /// offboarding is `Revoked`. Distinguished from
    /// [`Self::TransitionForbidden`] because no source status can ever reach
    /// it, so a client should not offer it at all. Maps to
    /// `InvalidComplianceTransition` (4006).
    TargetUnknownForbidden,
    /// The `from -> to` edge is not in the transition matrix (for example
    /// `Blocked -> Approved`, which must pass back through `Pending`). Maps
    /// to `InvalidComplianceTransition` (4006).
    TransitionForbidden,
    /// Batch pre-flight only: the same address appears more than once in the
    /// batch. Rejected so a batch cannot smuggle order-dependent compliance
    /// intent. Maps to `InvalidComplianceTransition` (4006).
    DuplicateUserInBatch,
}

impl TransitionGuard {
    /// Whether this verdict permits the transition.
    pub fn is_allowed(&self) -> bool {
        matches!(self, TransitionGuard::Allowed)
    }

    /// Whether this verdict is an authorization failure — the caller is the
    /// problem, not the requested edge. Lets a client route to "ask an
    /// authorized officer" instead of "pick a different status".
    pub fn is_authorization_failure(&self) -> bool {
        matches!(
            self,
            TransitionGuard::CallerUnauthorized | TransitionGuard::BlockedRequiresAdmin
        )
    }
}

/// The contract error a rejected verdict produces, or `None` when allowed.
///
/// This is the mapping that keeps a pre-flight read and a real invocation
/// telling the same story: whatever [`check_compliance_transition`] reports,
/// submitting the transition fails with exactly this code.
///
/// [`check_compliance_transition`]: AegisContract::check_compliance_transition
pub fn error_for_guard(guard: &TransitionGuard) -> Option<Error> {
    match guard {
        TransitionGuard::Allowed => None,
        TransitionGuard::NotInitialized => Some(Error::NotInitialized),
        TransitionGuard::ContractPaused => Some(Error::ContractPaused),
        TransitionGuard::CallerUnauthorized | TransitionGuard::BlockedRequiresAdmin => {
            Some(Error::Unauthorized)
        }
        TransitionGuard::StatusUnchanged => Some(Error::ComplianceStatusUnchanged),
        TransitionGuard::TargetUnknownForbidden
        | TransitionGuard::TransitionForbidden
        | TransitionGuard::DuplicateUserInBatch => Some(Error::InvalidComplianceTransition),
    }
}

/// Whether a rejected verdict aborts the invocation by **panicking** rather
/// than by returning `Err`.
///
/// Authorization and availability failures (`NotInitialized`,
/// `ContractPaused`, and both unauthorized variants) have always panicked in
/// this contract, and downstream tests, SDKs, and the fixture set depend on
/// that. Rule violations that are the caller's *choice* of edge
/// (`StatusUnchanged`, the forbidden-transition variants) are returned as
/// typed `Err` values. Keeping the split explicit here is what lets the guard
/// become the single evaluation without changing any existing failure shape.
pub fn guard_panics(guard: &TransitionGuard) -> bool {
    matches!(
        guard,
        TransitionGuard::NotInitialized
            | TransitionGuard::ContractPaused
            | TransitionGuard::CallerUnauthorized
            | TransitionGuard::BlockedRequiresAdmin
    )
}

// ─── Pre-flight report ────────────────────────────────────────────────────────

/// The full verdict for one proposed transition, as returned to clients.
///
/// Carries the resolved current status alongside the verdict so a caller
/// cannot race a separate `get_compliance_status` read against this one and
/// render an inconsistent pair.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComplianceTransitionCheck {
    /// The address whose status the transition would change.
    pub user: Address,
    /// The address that would sign and submit the transition.
    pub caller: Address,
    /// `user`'s status at read time (`Unknown` when no record exists).
    pub current_status: ComplianceStatus,
    /// The status the transition would move `user` to.
    pub requested_status: ComplianceStatus,
    /// Whether the transition would be committed if submitted now.
    pub allowed: bool,
    /// The first failing precondition, or `Allowed`.
    pub reason: TransitionGuard,
    /// The numeric error code a rejected submission would revert with, or
    /// `None` when allowed. Pre-resolved so clients can reuse their existing
    /// `docs/error-codes.md` mapping without duplicating the reason table.
    pub error_code: Option<u32>,
}

// ─── Evaluation ───────────────────────────────────────────────────────────────

/// Returns whether `caller` may authorize *any* transition away from `from`.
///
/// Mirrors the pause/unpause asymmetry in `admin.rs`: leaving `Blocked` is
/// admin-only so a compromised or coerced compliance officer cannot lift a
/// sanctions freeze. Every other transition — including *entering* `Blocked`
/// — is available to a ComplianceOfficer, an EmergencyOfficer, or the admin.
///
/// Pure read: unlike `admin::require_any_role`, it reports rather than panics,
/// so it is safe to call from a view entrypoint.
fn authority_guard(env: &Env, caller: &Address, from: &ComplianceStatus) -> TransitionGuard {
    let admin: Address = match env.storage().instance().get(&DataKey::Admin) {
        Some(admin) => admin,
        None => return TransitionGuard::NotInitialized,
    };

    if *caller == admin {
        return TransitionGuard::Allowed;
    }

    if from.is_blocked() {
        return TransitionGuard::BlockedRequiresAdmin;
    }

    match get_role(env, caller) {
        Role::ComplianceOfficer | Role::EmergencyOfficer => TransitionGuard::Allowed,
        _ => TransitionGuard::CallerUnauthorized,
    }
}

/// Evaluates every precondition for `caller` moving `user` to `new_status`.
///
/// **Never panics and never writes** — safe from both view entrypoints and
/// enforcement paths. Preconditions are applied in the order enforcement
/// applies them, and the first failure short-circuits:
///
/// 1. contract initialized,
/// 2. contract not paused,
/// 3. caller authority for the *current* status,
/// 4. requested status differs from the current one,
/// 5. the target is not `Unknown`,
/// 6. the `from -> to` edge is in the transition matrix.
///
/// One precondition is deliberately **not** evaluated: `require_auth`. Whether
/// the caller can actually produce a valid signature is a property of the
/// submitted transaction, not of ledger state, so a pre-flight `Allowed` means
/// "the rules permit this caller", not "this caller is authenticated".
pub fn evaluate_transition(
    env: &Env,
    caller: &Address,
    user: &Address,
    new_status: &ComplianceStatus,
) -> TransitionGuard {
    let current = get_compliance_status(env, user);
    evaluate_from_status(env, caller, &current, new_status)
}

/// [`evaluate_transition`] against an already-resolved current status.
///
/// Used by enforcement paths that have already read the status (so the guard
/// cannot be evaluated against a different one than the write applies to) and
/// by the matrix tests, which walk source statuses directly.
pub fn evaluate_from_status(
    env: &Env,
    caller: &Address,
    current: &ComplianceStatus,
    new_status: &ComplianceStatus,
) -> TransitionGuard {
    if is_paused(env) {
        // Checked before authority so a paused contract reports the pause
        // rather than leaking whether the caller would otherwise qualify.
        return TransitionGuard::ContractPaused;
    }

    let authority = authority_guard(env, caller, current);
    if !authority.is_allowed() {
        return authority;
    }

    if current == new_status {
        return TransitionGuard::StatusUnchanged;
    }
    if *new_status == ComplianceStatus::Unknown {
        return TransitionGuard::TargetUnknownForbidden;
    }
    if !transition_is_allowed(current, new_status) {
        return TransitionGuard::TransitionForbidden;
    }

    TransitionGuard::Allowed
}

/// Builds the client-facing report for a proposed transition.
pub fn check_transition(
    env: &Env,
    caller: &Address,
    user: &Address,
    new_status: &ComplianceStatus,
) -> ComplianceTransitionCheck {
    let current_status = get_compliance_status(env, user);
    let reason = evaluate_from_status(env, caller, &current_status, new_status);

    ComplianceTransitionCheck {
        user: user.clone(),
        caller: caller.clone(),
        current_status,
        requested_status: *new_status,
        allowed: reason.is_allowed(),
        reason,
        error_code: error_for_guard(&reason).map(|err| err as u32),
    }
}

// ─── Enforcement entry point ──────────────────────────────────────────────────

/// Enforces the guard for a transition, returning the current status on
/// success. **This is the only path a state-changing compliance call may use
/// to reach a verdict**, so enforcement and pre-flight can never disagree.
///
/// Rejected verdicts either panic or return `Err`, per [`guard_panics`],
/// preserving the failure shape each precondition had before the guard
/// existed.
pub fn require_transition(
    env: &Env,
    caller: &Address,
    current: &ComplianceStatus,
    new_status: &ComplianceStatus,
) -> Result<(), Error> {
    let guard = evaluate_from_status(env, caller, current, new_status);
    if guard.is_allowed() {
        return Ok(());
    }

    // `error_for_guard` returns `None` only for `Allowed`, handled above.
    let error = match error_for_guard(&guard) {
        Some(error) => error,
        None => return Ok(()),
    };

    if guard_panics(&guard) {
        soroban_sdk::panic_with_error!(env, error);
    }

    Err(error)
}

/// Enforces only the *authority* half of the guard, for the two legacy
/// entrypoints (`whitelist_user` / `revoke_whitelist`) whose documented
/// behaviour is to tolerate a no-op rather than reject it. They still must not
/// tolerate an unauthorized caller.
pub fn require_transition_authority(env: &Env, caller: &Address, current: &ComplianceStatus) {
    let guard = authority_guard(env, caller, current);
    if guard.is_allowed() {
        return;
    }
    if let Some(error) = error_for_guard(&guard) {
        soroban_sdk::panic_with_error!(env, error);
    }
}

// ─── Batch pre-flight ─────────────────────────────────────────────────────────

/// Whether `updates[index]` repeats an address that appears earlier in the
/// batch. Matches the duplicate rule `batch_set_compliance_status` enforces.
fn is_duplicate_at(updates: &Vec<ComplianceBatchUpdate>, index: u32) -> bool {
    let current = updates.get(index).unwrap();
    for earlier in 0..index {
        if updates.get(earlier).unwrap().user == current.user {
            return true;
        }
    }
    false
}

/// Evaluates every entry of a proposed batch independently.
///
/// Entries are independent because duplicate addresses are rejected outright:
/// with no address appearing twice, no entry can change the current status
/// another entry is evaluated against. An entry that repeats an earlier
/// address reports [`TransitionGuard::DuplicateUserInBatch`].
///
/// The batch is **atomic** on submission, so a single rejected entry fails the
/// whole call. Clients should treat any `allowed == false` row as "this batch
/// will not commit", not as "this row will be skipped".
pub fn check_batch(
    env: &Env,
    caller: &Address,
    updates: &Vec<ComplianceBatchUpdate>,
) -> Vec<ComplianceTransitionCheck> {
    let mut out = vec![env];

    for index in 0..updates.len() {
        let update = updates.get(index).unwrap();
        let mut check = check_transition(env, caller, &update.user, &update.new_status);

        if is_duplicate_at(updates, index) {
            check.allowed = false;
            check.reason = TransitionGuard::DuplicateUserInBatch;
            check.error_code = error_for_guard(&check.reason).map(|err| err as u32);
        }

        out.push_back(check);
    }

    out
}

// ─── Public API ───────────────────────────────────────────────────────────────

#[contractimpl]
impl AegisContract {
    /// Returns whether `caller` could move `user` to `new_status` right now,
    /// and the precise reason when it could not.
    ///
    /// Pure read: requires no authorization, changes nothing, never reverts,
    /// and stays callable while the contract is paused (it reports the pause
    /// as the reason). The verdict is produced by the same evaluation the
    /// state-changing entrypoints use, so `allowed == false` guarantees a
    /// submission would fail with `error_code`.
    ///
    /// Point-in-time only — the state it reads can change before a
    /// transaction lands. See `docs/compliance-transition-guards.md`.
    pub fn check_compliance_transition(
        env: Env,
        caller: Address,
        user: Address,
        new_status: ComplianceStatus,
    ) -> ComplianceTransitionCheck {
        check_transition(&env, &caller, &user, &new_status)
    }

    /// Returns the guard verdict alone for a proposed transition, for clients
    /// that only need to branch on the reason. Equivalent to the `reason`
    /// field of [`Self::check_compliance_transition`].
    pub fn get_compliance_transition_guard(
        env: Env,
        caller: Address,
        user: Address,
        new_status: ComplianceStatus,
    ) -> TransitionGuard {
        evaluate_transition(&env, &caller, &user, &new_status)
    }

    /// Pre-flights every entry of a `batch_set_compliance_status` call,
    /// returning one verdict per entry in input order.
    ///
    /// The batch commits only if **every** entry is `allowed`; a single
    /// rejection fails the whole submission. Pure read — never reverts, even
    /// for an empty batch (which returns an empty vector, matching the
    /// batch entrypoint's `0` result).
    pub fn check_compliance_batch(
        env: Env,
        caller: Address,
        updates: Vec<ComplianceBatchUpdate>,
    ) -> Vec<ComplianceTransitionCheck> {
        check_batch(&env, &caller, &updates)
    }
}
