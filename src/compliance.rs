//! Compliance lifecycle state machine.
//!
//! Investor compliance is modelled as an explicit five-state lifecycle rather
//! than a single whitelist boolean, so the contract can distinguish between
//! "never seen", "KYC in review", "cleared", "clearance withdrawn", and
//! "sanctioned/frozen". Every state change is validated against a fixed
//! transition matrix, is role-gated, and emits a `compliance_status_changed`
//! event. Minting and transfers consume the lifecycle state directly.
//!
//! See `docs/compliance-lifecycle.md` for the full specification.

use soroban_sdk::{contractimpl, contracttype, vec, Address, Env, Vec};

use crate::admin::{get_admin, require_any_role, require_not_paused};
use crate::{AegisContract, AegisContractArgs, AegisContractClient, DataKey, Error, Role};

// ─── Lifecycle state ──────────────────────────────────────────────────────────

/// Compliance lifecycle state for a single investor address.
///
/// The variant order is part of the contract's ABI: variants are append-only
/// and must never be reordered or repurposed (same stability contract as
/// `docs/error-codes.md` numeric codes and `docs/events.md` topics).
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComplianceStatus {
    /// No compliance record exists for this address. The default for every
    /// address the registry has never seen. Nothing is permitted: the address
    /// can neither send nor receive.
    Unknown,
    /// KYC/AML review has started but has not cleared. The address is known to
    /// the registry but still cannot send or receive — a deliberate
    /// fail-closed state so an in-flight review never grants access.
    Pending,
    /// Compliance cleared. The only state in which an address may hold, send,
    /// or receive assets.
    Approved,
    /// Clearance was withdrawn (expired documents, lapsed attestation,
    /// investor offboarding). The address keeps any existing balance but can
    /// neither send nor receive until it is re-approved.
    Revoked,
    /// Sanctioned or frozen by an enforcement action. Like `Revoked`, but
    /// escalated: only the supreme admin may move an address out of `Blocked`,
    /// and it can only be moved into re-review (`Pending`), never straight
    /// back to `Approved`.
    Blocked,
}

impl ComplianceStatus {
    /// Whether this state permits sending, receiving, and holding assets.
    /// `Approved` is the only permissive state.
    pub fn is_approved(&self) -> bool {
        matches!(self, ComplianceStatus::Approved)
    }

    /// Whether this state is an enforcement freeze that only the supreme admin
    /// can lift.
    pub fn is_blocked(&self) -> bool {
        matches!(self, ComplianceStatus::Blocked)
    }
}

// ─── Events ───────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct UserWhitelistedEvent {
    pub caller: Address,
    pub user: Address,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct WhitelistRevokedEvent {
    pub caller: Address,
    pub user: Address,
}

/// Emitted on every committed compliance lifecycle transition, including the
/// ones driven by the legacy `whitelist_user` / `revoke_whitelist` wrappers.
/// This is the canonical signal for compliance indexers; the two legacy events
/// above are retained only for backwards compatibility.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ComplianceStatusChangedEvent {
    pub caller: Address,
    pub user: Address,
    pub previous_status: ComplianceStatus,
    pub new_status: ComplianceStatus,
}

// ─── Transition matrix ────────────────────────────────────────────────────────

/// Returns whether `from -> to` is a permitted lifecycle transition.
///
/// | From       | Allowed targets                     |
/// |------------|-------------------------------------|
/// | `Unknown`  | `Pending`, `Approved`, `Blocked`    |
/// | `Pending`  | `Approved`, `Revoked`, `Blocked`    |
/// | `Approved` | `Pending`, `Revoked`, `Blocked`     |
/// | `Revoked`  | `Pending`, `Approved`, `Blocked`    |
/// | `Blocked`  | `Pending` (supreme admin only)      |
///
/// Two rules hold globally:
///
/// * **No self-transitions.** `from == to` is rejected with
///   `ComplianceStatusUnchanged` so a no-op never emits a misleading event.
/// * **`Unknown` is never a target.** Compliance history cannot be erased;
///   offboarding is `Revoked`, not a reset to "never seen".
pub fn transition_is_allowed(from: &ComplianceStatus, to: &ComplianceStatus) -> bool {
    use ComplianceStatus::*;

    // A no-op is not a transition, and history is never erased.
    if from == to || *to == Unknown {
        return false;
    }

    match from {
        Unknown => matches!(to, Pending | Approved | Blocked),
        Pending => matches!(to, Approved | Revoked | Blocked),
        Approved => matches!(to, Pending | Revoked | Blocked),
        Revoked => matches!(to, Pending | Approved | Blocked),
        // Quarantine: the only exit is back into review, and only the supreme
        // admin may authorize it (see `require_transition_authority`).
        Blocked => matches!(to, Pending),
    }
}

/// Returns every state reachable from `from` in one transition, in ABI order.
pub fn allowed_transitions(env: &Env, from: &ComplianceStatus) -> Vec<ComplianceStatus> {
    let candidates = [
        ComplianceStatus::Unknown,
        ComplianceStatus::Pending,
        ComplianceStatus::Approved,
        ComplianceStatus::Revoked,
        ComplianceStatus::Blocked,
    ];

    let mut out = vec![env];
    for candidate in candidates.iter() {
        if transition_is_allowed(from, candidate) {
            out.push_back(*candidate);
        }
    }
    out
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Returns the compliance lifecycle status for `user`.
///
/// Absent storage means [`ComplianceStatus::Unknown`] — the fail-closed
/// default. Pure read: never panics, never writes.
pub fn get_compliance_status(env: &Env, user: &Address) -> ComplianceStatus {
    env.storage()
        .persistent()
        .get(&DataKey::ComplianceStatus(user.clone()))
        .unwrap_or(ComplianceStatus::Unknown)
}

/// Internal helper to check approval status across modules.
///
/// Kept as `is_whitelisted` for backwards compatibility: it is now derived
/// from the lifecycle rather than from a standalone boolean, and is `true`
/// only for [`ComplianceStatus::Approved`].
pub fn is_whitelisted(env: &Env, user: &Address) -> bool {
    get_compliance_status(env, user).is_approved()
}

/// Persists a lifecycle status and keeps the legacy `Whitelist` mirror in
/// sync so any consumer still reading the old key cannot observe a state the
/// lifecycle disagrees with.
fn write_status(env: &Env, user: &Address, status: &ComplianceStatus) {
    env.storage()
        .persistent()
        .set(&DataKey::ComplianceStatus(user.clone()), status);

    if status.is_approved() {
        env.storage()
            .persistent()
            .set(&DataKey::Whitelist(user.clone()), &true);
    } else {
        env.storage()
            .persistent()
            .remove(&DataKey::Whitelist(user.clone()));
    }
}

/// Authorization for a lifecycle transition.
///
/// Leaving `Blocked` is restricted to the supreme admin, mirroring the
/// pause/unpause asymmetry in `admin.rs`: a compromised or coerced compliance
/// officer must not be able to lift a sanctions freeze. Every other
/// transition — including *entering* `Blocked` — is available to a
/// ComplianceOfficer, an EmergencyOfficer, or the admin.
fn require_transition_authority(env: &Env, caller: &Address, from: &ComplianceStatus) {
    if from.is_blocked() {
        if *caller != get_admin(env) {
            soroban_sdk::panic_with_error!(env, Error::Unauthorized);
        }
        return;
    }

    require_any_role(
        env,
        caller,
        &[Role::ComplianceOfficer, Role::EmergencyOfficer],
    );
}

/// Applies a validated transition and emits `compliance_status_changed`.
/// Assumes the caller has already been authorized.
fn apply_transition(
    env: &Env,
    caller: &Address,
    user: &Address,
    previous: ComplianceStatus,
    new_status: ComplianceStatus,
) {
    write_status(env, user, &new_status);

    env.events().publish(
        ("compliance_status_changed",),
        ComplianceStatusChangedEvent {
            caller: caller.clone(),
            user: user.clone(),
            previous_status: previous,
            new_status,
        },
    );
}

// ─── Enforcement helpers consumed by minting and transfers ────────────────────

/// Asserts that `user` may receive assets (mint or incoming transfer).
///
/// Maps the lifecycle state onto a granular error code so a client can tell a
/// never-onboarded investor ("start KYC") from one under review ("wait") from
/// a sanctioned one ("do not retry"):
///
/// | Status               | Error                          |
/// |----------------------|--------------------------------|
/// | `Approved`           | — (permitted)                  |
/// | `Blocked`            | `ReceiverBlocked` (4003)       |
/// | `Pending`            | `ReceiverCompliancePending` (4005) |
/// | `Unknown`, `Revoked` | `ReceiverNotWhitelisted` (4001) |
pub fn require_can_receive(env: &Env, user: &Address) -> Result<(), Error> {
    match get_compliance_status(env, user) {
        ComplianceStatus::Approved => Ok(()),
        ComplianceStatus::Blocked => Err(Error::ReceiverBlocked),
        ComplianceStatus::Pending => Err(Error::ReceiverCompliancePending),
        ComplianceStatus::Unknown | ComplianceStatus::Revoked => Err(Error::ReceiverNotWhitelisted),
    }
}

/// Asserts that `user` may send assets. Mirror of [`require_can_receive`]
/// using the sender-side codes `SenderBlocked` (4002),
/// `SenderCompliancePending` (4004), and `SenderNotWhitelisted` (4000).
pub fn require_can_send(env: &Env, user: &Address) -> Result<(), Error> {
    match get_compliance_status(env, user) {
        ComplianceStatus::Approved => Ok(()),
        ComplianceStatus::Blocked => Err(Error::SenderBlocked),
        ComplianceStatus::Pending => Err(Error::SenderCompliancePending),
        ComplianceStatus::Unknown | ComplianceStatus::Revoked => Err(Error::SenderNotWhitelisted),
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

#[contractimpl]
impl AegisContract {
    /// Returns the compliance lifecycle status for `user`
    /// (`Unknown` when no record exists). Pure read; always available.
    pub fn get_compliance_status(env: Env, user: Address) -> ComplianceStatus {
        get_compliance_status(&env, &user)
    }

    /// Returns whether `from -> to` is a permitted lifecycle transition.
    /// Pure read; requires no authorization and never reverts, so clients can
    /// pre-flight a transition before building a transaction.
    pub fn is_compliance_transition_allowed(
        env: Env,
        from: ComplianceStatus,
        to: ComplianceStatus,
    ) -> bool {
        let _ = &env;
        transition_is_allowed(&from, &to)
    }

    /// Returns every state reachable from `from` in a single transition.
    /// Pure read; lets a dashboard render only the legal next actions instead
    /// of hardcoding the matrix.
    pub fn get_allowed_transitions(env: Env, from: ComplianceStatus) -> Vec<ComplianceStatus> {
        allowed_transitions(&env, &from)
    }

    /// Returns every state reachable in one transition from `user`'s *current*
    /// status. Convenience wrapper over `get_allowed_transitions`.
    pub fn get_allowed_transitions_for(env: Env, user: Address) -> Vec<ComplianceStatus> {
        let current = get_compliance_status(&env, &user);
        allowed_transitions(&env, &current)
    }

    /// Moves `user` to `new_status`, enforcing the lifecycle transition matrix.
    ///
    /// Requires the ComplianceOfficer role, EmergencyOfficer role, or Admin —
    /// except when the address is currently `Blocked`, where only the supreme
    /// admin may act. Blocked when the contract is paused.
    ///
    /// Reverts with `ComplianceStatusUnchanged` (4007) for a no-op and
    /// `InvalidComplianceTransition` (4006) for a transition the matrix
    /// forbids. Emits `compliance_status_changed` on success.
    pub fn set_compliance_status(
        env: Env,
        caller: Address,
        user: Address,
        new_status: ComplianceStatus,
    ) -> Result<(), Error> {
        require_not_paused(&env);
        caller.require_auth();

        let current = get_compliance_status(&env, &user);
        require_transition_authority(&env, &caller, &current);

        if current == new_status {
            return Err(Error::ComplianceStatusUnchanged);
        }
        if !transition_is_allowed(&current, &new_status) {
            return Err(Error::InvalidComplianceTransition);
        }

        apply_transition(&env, &caller, &user, current, new_status);

        Ok(())
    }

    /// Approves `user` for compliance (legacy alias for a transition to
    /// [`ComplianceStatus::Approved`]).
    ///
    /// Requires the ComplianceOfficer role, EmergencyOfficer role, or Admin.
    /// Blocked when the contract is paused. Reverts with
    /// `InvalidComplianceTransition` if `user` is `Blocked` — a sanctions
    /// freeze cannot be lifted through the legacy path.
    ///
    /// Idempotent: re-approving an already-`Approved` address succeeds without
    /// emitting a lifecycle event, preserving the pre-lifecycle behaviour.
    /// Always emits `user_whitelisted` for backwards compatibility.
    pub fn whitelist_user(env: Env, admin: Address, user: Address) -> Result<(), Error> {
        require_not_paused(&env);
        admin.require_auth();

        let current = get_compliance_status(&env, &user);
        require_transition_authority(&env, &admin, &current);

        if current != ComplianceStatus::Approved {
            if !transition_is_allowed(&current, &ComplianceStatus::Approved) {
                return Err(Error::InvalidComplianceTransition);
            }
            apply_transition(&env, &admin, &user, current, ComplianceStatus::Approved);
        }

        env.events().publish(
            ("user_whitelisted",),
            UserWhitelistedEvent {
                caller: admin,
                user,
            },
        );

        Ok(())
    }

    /// Revokes `user`'s compliance clearance (legacy alias for a transition to
    /// [`ComplianceStatus::Revoked`]).
    ///
    /// Requires the ComplianceOfficer role, EmergencyOfficer role, or Admin.
    /// Blocked when the contract is paused.
    ///
    /// Tolerant by design, matching the pre-lifecycle behaviour: revoking an
    /// `Unknown` address is a no-op rather than an error, and revoking a
    /// `Blocked` address leaves the stronger `Blocked` state intact rather
    /// than silently downgrading an enforcement freeze. Always emits
    /// `whitelist_revoked` for backwards compatibility.
    pub fn revoke_whitelist(env: Env, admin: Address, user: Address) -> Result<(), Error> {
        require_not_paused(&env);
        admin.require_auth();

        let current = get_compliance_status(&env, &user);
        require_transition_authority(&env, &admin, &current);

        // `Unknown` has nothing to revoke; `Blocked` already exceeds
        // `Revoked` and must not be downgraded by a legacy call.
        if transition_is_allowed(&current, &ComplianceStatus::Revoked) {
            apply_transition(&env, &admin, &user, current, ComplianceStatus::Revoked);
        }

        env.events().publish(
            ("whitelist_revoked",),
            WhitelistRevokedEvent {
                caller: admin,
                user,
            },
        );

        Ok(())
    }
}
