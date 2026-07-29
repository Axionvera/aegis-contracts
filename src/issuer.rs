//! Issuer role separation.
//!
//! The RBAC in [`crate::admin`] answers *which* privileges an address holds.
//! This module answers the separation-of-duties question that sits on top of
//! it: **may the same key both clear an investor and issue that investor
//! units?**
//!
//! Under the base role model the answer is yes. The supreme admin bypasses
//! every role check, so a single key can approve an address and then mint to
//! it with no second party involved. For an operational convenience that is
//! fine; for an RWA issuer it is the classic control failure — the party that
//! decides *who may hold* the asset must not also be the party that decides
//! *who receives* it.
//!
//! Duties here are derived from what the contract **enforces**, not from what
//! a role is named after. `mint_asset` and `distribute_yield` call
//! `require_role(AssetManager)`, which admits only an `AssetManager` or the
//! admin, so `EmergencyOfficer` carries compliance and pause authority but
//! **not** issuance. Modelling it otherwise would describe a privilege that
//! does not exist, and a separation control built on an inaccurate privilege
//! map is worse than none.
//!
//! This module makes that separation enforceable **without changing any
//! existing deployment's behaviour**. Separation is an opt-in policy: until an
//! admin enables it, [`IssuerSeparationPolicy::default_policy`] permits
//! everything the contract permitted before. Once enabled, each control can be
//! relaxed independently, so an issuer can adopt the parts their operating
//! model supports:
//!
//! | Control | What it blocks |
//! |---|---|
//! | `allow_dual_duty_issuance: false` | A caller holding *both* compliance and issuance duties may not mint. |
//! | `allow_self_issuance: false` | A caller may not mint to their own address. |
//! | `require_independent_approver: true` | The caller who approved a recipient's compliance may not mint to that recipient (four-eyes). |
//!
//! The policy is admin-governed and never self-locking: `set_issuer_separation_policy`
//! is not itself gated by the policy, so an admin can always relax a rule that
//! turns out to be too strict. See `docs/issuer-role-separation.md`.

// The legacy `Events::publish((topic,), payload)` API is used intentionally:
// docs/events.md freezes these (topic, payload) shapes as a stable off-chain
// contract, and src/test.rs asserts them exactly.
#![allow(deprecated)]

use soroban_sdk::{contractimpl, contracttype, vec, Address, Env, Vec};

use crate::admin::{get_role, require_not_paused};
use crate::{AegisContract, AegisContractArgs, AegisContractClient, DataKey, Error, Role};

// ─── Duties ───────────────────────────────────────────────────────────────────

/// A class of privilege, independent of which role happens to carry it.
///
/// Roles are the unit of *assignment*; duties are the unit of *separation*.
/// The distinction matters because one role can carry several duties — the
/// admin carries all of them — and it is the *combination* of `Compliance`
/// and `Issuance` in one key that separation-of-duties controls exist to
/// detect.
///
/// The variant order is part of the contract's ABI: variants are append-only
/// and must never be reordered or repurposed.
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssuerDuty {
    /// Deciding who may hold the asset: the compliance lifecycle and the
    /// whitelist (`set_compliance_status`, `whitelist_user`, …).
    Compliance,
    /// Deciding who receives units and how many: `mint_asset`,
    /// `distribute_yield`.
    Issuance,
    /// Halting the protocol: `pause`. Held by `EmergencyOfficer` and the
    /// admin.
    Emergency,
    /// Changing the rules themselves: roles, caps, protocol config, and
    /// lifting a pause. Held only by the admin.
    Governance,
}

/// Whether `role` carries `duty`.
///
/// `Role::Admin` carries every duty by construction — the admin bypasses role
/// checks throughout the contract, so modelling it as anything narrower here
/// would describe a restriction that does not exist.
pub fn role_has_duty(role: &Role, duty: &IssuerDuty) -> bool {
    match role {
        Role::None => false,
        Role::Admin => true,
        Role::ComplianceOfficer => matches!(duty, IssuerDuty::Compliance),
        Role::AssetManager => matches!(duty, IssuerDuty::Issuance),
        // Compliance plus the pause switch — *not* issuance: `mint_asset`
        // requires `AssetManager` specifically. See the module docs.
        Role::EmergencyOfficer => {
            matches!(duty, IssuerDuty::Compliance | IssuerDuty::Emergency)
        }
    }
}

/// Every duty `role` carries, in ABI order. Empty for `Role::None`.
pub fn duties_of_role(env: &Env, role: &Role) -> Vec<IssuerDuty> {
    let all = [
        IssuerDuty::Compliance,
        IssuerDuty::Issuance,
        IssuerDuty::Emergency,
        IssuerDuty::Governance,
    ];

    let mut out = vec![env];
    for duty in all.iter() {
        if role_has_duty(role, duty) {
            out.push_back(*duty);
        }
    }
    out
}

/// The effective role of `caller`, treating the supreme admin as `Role::Admin`
/// whatever the role table says.
///
/// The admin's authority comes from `DataKey::Admin`, not from a role
/// assignment, so a duty check that consulted only `get_role` would understate
/// what the admin can actually do — and separation controls that understate
/// authority are worse than none.
///
/// Returns `Role::None` on an uninitialized contract rather than panicking, so
/// the read entrypoints stay panic-free.
pub fn effective_role(env: &Env, caller: &Address) -> Role {
    match env
        .storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
    {
        Some(admin) if admin == *caller => Role::Admin,
        Some(_) => get_role(env, caller),
        None => Role::None,
    }
}

// ─── Policy ───────────────────────────────────────────────────────────────────

/// The deployment's separation-of-duties configuration.
///
/// Every field defaults to the permissive value, so a contract that has never
/// called `set_issuer_separation_policy` behaves exactly as it did before this
/// module existed. Enabling separation is a deliberate, audited act.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssuerSeparationPolicy {
    /// Master switch. While `false`, every other field is inert and issuance
    /// is governed by the role check alone.
    pub enforced: bool,
    /// Whether an address holding **both** the compliance and issuance duties
    /// may issue. Today only the admin holds both, so this is in practice the
    /// control that forces the admin to delegate issuance to a dedicated
    /// `AssetManager` key.
    ///
    /// Setting this `false` is the core separation control: it forces issuance
    /// through a key that cannot also alter the whitelist. It restricts the
    /// **admin** deliberately — an unrestricted admin makes the control
    /// decorative. The admin can still lift the policy, so this is a control
    /// against routine key misuse and operator error, not a defence against a
    /// compromised admin key. See `docs/admin-misuse-risks.md`.
    pub allow_dual_duty_issuance: bool,
    /// Whether an issuer may mint to their own address. Self-issuance is the
    /// shortest path from "issuance key" to "holder of the asset".
    pub allow_self_issuance: bool,
    /// Whether the caller who last approved a recipient's compliance may issue
    /// to that recipient. Setting this `true` enforces four-eyes: whoever
    /// cleared the investor cannot be the one who funds them.
    ///
    /// Only the **most recent** approver is recorded, so this is a control
    /// against one key performing both steps, not a full historical audit —
    /// reconstruct that from `compliance_status_changed` events.
    pub require_independent_approver: bool,
}

impl IssuerSeparationPolicy {
    /// The permissive default applied when no policy has been stored: separation
    /// off, every control relaxed. Chosen so adding this module changes no
    /// existing deployment's behaviour.
    pub fn default_policy() -> Self {
        IssuerSeparationPolicy {
            enforced: false,
            allow_dual_duty_issuance: true,
            allow_self_issuance: true,
            require_independent_approver: false,
        }
    }
}

/// Returns the active policy, or the permissive default when none is stored.
/// Pure read: never panics, never writes.
pub fn get_policy(env: &Env) -> IssuerSeparationPolicy {
    env.storage()
        .instance()
        .get(&DataKey::IssuerSeparationPolicy)
        .unwrap_or_else(IssuerSeparationPolicy::default_policy)
}

// ─── Approver record ──────────────────────────────────────────────────────────

/// Records that `approver` moved `user` into `Approved`.
///
/// Called from the compliance lifecycle writer on every committed transition
/// *into* `Approved`. The record is intentionally **not** cleared when the
/// address later leaves `Approved`: a revoked investor who is re-approved by a
/// different officer must overwrite it, but a revocation alone should not
/// erase who granted the clearance being revoked.
pub fn record_compliance_approver(env: &Env, user: &Address, approver: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::ComplianceApprover(user.clone()), approver);
}

/// Returns the address that last approved `user`'s compliance, if any.
/// Pure read: never panics, never writes.
pub fn get_compliance_approver(env: &Env, user: &Address) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::ComplianceApprover(user.clone()))
}

// ─── Events ───────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct IssuerSeparationPolicyUpdatedEvent {
    pub admin: Address,
    pub previous_policy: IssuerSeparationPolicy,
    pub new_policy: IssuerSeparationPolicy,
}

// ─── Guard ────────────────────────────────────────────────────────────────────

/// Why an issuance is permitted or refused under the separation policy.
///
/// Exactly one reason is returned: the **first** control that fails, in the
/// order enforcement applies them. Variants are append-only ABI.
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssuanceGuard {
    /// Every control passes.
    Allowed,
    /// The contract has not been initialized. Maps to `NotInitialized` (2000).
    NotInitialized,
    /// The caller does not carry the issuance duty at all — a plain RBAC
    /// failure, not a separation one. Maps to `Unauthorized` (3000).
    MissingIssuanceDuty,
    /// The caller carries both the compliance and issuance duties while
    /// `allow_dual_duty_issuance` is `false`. Maps to `IssuanceDutyConflict`
    /// (3007).
    DualDutyConflict,
    /// The caller is the recipient while `allow_self_issuance` is `false`.
    /// Maps to `SelfIssuanceForbidden` (3008).
    SelfIssuanceForbidden,
    /// The caller is the recorded approver of the recipient's compliance while
    /// `require_independent_approver` is `true`. Maps to
    /// `IssuanceApproverConflict` (3009).
    ApproverConflict,
}

impl IssuanceGuard {
    /// Whether this verdict permits the issuance.
    pub fn is_allowed(&self) -> bool {
        matches!(self, IssuanceGuard::Allowed)
    }

    /// Whether the refusal comes from the separation policy rather than from
    /// the base role model. Lets a client distinguish "this key is not an
    /// issuer" from "this key is an issuer, but not for *this* recipient".
    pub fn is_separation_failure(&self) -> bool {
        matches!(
            self,
            IssuanceGuard::DualDutyConflict
                | IssuanceGuard::SelfIssuanceForbidden
                | IssuanceGuard::ApproverConflict
        )
    }
}

/// The contract error a refused verdict produces, or `None` when allowed.
pub fn error_for_guard(guard: &IssuanceGuard) -> Option<Error> {
    match guard {
        IssuanceGuard::Allowed => None,
        IssuanceGuard::NotInitialized => Some(Error::NotInitialized),
        IssuanceGuard::MissingIssuanceDuty => Some(Error::Unauthorized),
        IssuanceGuard::DualDutyConflict => Some(Error::IssuanceDutyConflict),
        IssuanceGuard::SelfIssuanceForbidden => Some(Error::SelfIssuanceForbidden),
        IssuanceGuard::ApproverConflict => Some(Error::IssuanceApproverConflict),
    }
}

/// Evaluates the separation controls for `caller` issuing to `recipient`.
///
/// Pass `None` for `recipient` for an issuance with no single beneficiary
/// (`distribute_yield`); the recipient-scoped controls are then skipped, since
/// there is no address for them to be about.
///
/// **Never panics and never writes** — safe from view entrypoints and from the
/// enforcement path, which is what keeps the pre-flight read
/// (`check_issuance_authority`) and `mint_asset` in agreement.
pub fn evaluate_issuance(
    env: &Env,
    caller: &Address,
    recipient: Option<&Address>,
) -> IssuanceGuard {
    if !env.storage().instance().has(&DataKey::Admin) {
        return IssuanceGuard::NotInitialized;
    }

    let role = effective_role(env, caller);
    if !role_has_duty(&role, &IssuerDuty::Issuance) {
        return IssuanceGuard::MissingIssuanceDuty;
    }

    let policy = get_policy(env);
    if !policy.enforced {
        return IssuanceGuard::Allowed;
    }

    if !policy.allow_dual_duty_issuance && role_has_duty(&role, &IssuerDuty::Compliance) {
        return IssuanceGuard::DualDutyConflict;
    }

    let recipient = match recipient {
        Some(recipient) => recipient,
        // No beneficiary: the recipient-scoped controls do not apply.
        None => return IssuanceGuard::Allowed,
    };

    if !policy.allow_self_issuance && *caller == *recipient {
        return IssuanceGuard::SelfIssuanceForbidden;
    }

    if policy.require_independent_approver {
        if let Some(approver) = get_compliance_approver(env, recipient) {
            if approver == *caller {
                return IssuanceGuard::ApproverConflict;
            }
        }
    }

    IssuanceGuard::Allowed
}

/// Enforces the separation controls, panicking with the mapped error on
/// refusal. Authorization failures have always aborted by panicking in this
/// contract; keeping that shape means adding these controls changes no
/// existing SDK error handling.
pub fn require_issuance_authority(env: &Env, caller: &Address, recipient: Option<&Address>) {
    let guard = evaluate_issuance(env, caller, recipient);
    if guard.is_allowed() {
        return;
    }
    if let Some(error) = error_for_guard(&guard) {
        soroban_sdk::panic_with_error!(env, error);
    }
}

// ─── Read response ────────────────────────────────────────────────────────────

/// The full separation verdict for a proposed issuance, as returned to clients.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssuanceAuthorityCheck {
    /// The address that would sign and submit the issuance.
    pub caller: Address,
    /// The address that would receive the units.
    pub recipient: Address,
    /// `caller`'s effective role (`Admin` for the supreme admin, whatever the
    /// role table holds).
    pub caller_role: Role,
    /// Every duty `caller_role` carries. A caller carrying both `Compliance`
    /// and `Issuance` is the condition `allow_dual_duty_issuance` governs.
    pub caller_duties: Vec<IssuerDuty>,
    /// The address that last approved `recipient`'s compliance, if any.
    pub recipient_approver: Option<Address>,
    /// Whether the issuance would clear the separation controls now.
    pub allowed: bool,
    /// The first failing control, or `Allowed`.
    pub reason: IssuanceGuard,
    /// The numeric error code a refused submission would revert with.
    pub error_code: Option<u32>,
}

/// Builds the client-facing separation report for a proposed issuance.
pub fn check_issuance(env: &Env, caller: &Address, recipient: &Address) -> IssuanceAuthorityCheck {
    let caller_role = effective_role(env, caller);
    let reason = evaluate_issuance(env, caller, Some(recipient));

    IssuanceAuthorityCheck {
        caller: caller.clone(),
        recipient: recipient.clone(),
        caller_duties: duties_of_role(env, &caller_role),
        caller_role,
        recipient_approver: get_compliance_approver(env, recipient),
        allowed: reason.is_allowed(),
        reason,
        error_code: error_for_guard(&reason).map(|err| err as u32),
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

#[contractimpl]
impl AegisContract {
    /// Returns the active issuer separation policy, or the permissive default
    /// when none has been set. Pure read; always available.
    pub fn get_issuer_separation_policy(env: Env) -> IssuerSeparationPolicy {
        get_policy(&env)
    }

    /// Replaces the issuer separation policy. Admin-only; blocked while the
    /// contract is paused.
    ///
    /// Applied in a single call rather than through 2-step governance: unlike
    /// a supply cap, tightening separation cannot strand value, and an issuer
    /// responding to a suspected key compromise should not have to wait a
    /// second transaction to close the gap. Loosening it is equally immediate,
    /// which is what guarantees the policy can never lock a deployment out of
    /// issuance — see `docs/issuer-role-separation.md`.
    ///
    /// Emits `issuer_separation_policy_updated` with both the previous and the
    /// new policy, so an auditor can reconstruct when each control was in force
    /// without replaying storage.
    pub fn set_issuer_separation_policy(
        env: Env,
        admin: Address,
        policy: IssuerSeparationPolicy,
    ) -> Result<(), Error> {
        require_not_paused(&env);
        admin.require_auth();
        if admin != crate::admin::get_admin(&env) {
            return Err(Error::Unauthorized);
        }

        let previous_policy = get_policy(&env);
        env.storage()
            .instance()
            .set(&DataKey::IssuerSeparationPolicy, &policy);

        env.events().publish(
            ("issuer_separation_policy_updated",),
            IssuerSeparationPolicyUpdatedEvent {
                admin,
                previous_policy,
                new_policy: policy,
            },
        );

        Ok(())
    }

    /// Returns the duties carried by `role`. Pure read; the duty table is
    /// fixed for a contract build, so clients may cache it.
    pub fn get_role_duties(env: Env, role: Role) -> Vec<IssuerDuty> {
        duties_of_role(&env, &role)
    }

    /// Returns the duties `address` currently carries, resolving the supreme
    /// admin to `Role::Admin`. Pure read.
    pub fn get_duties_of(env: Env, address: Address) -> Vec<IssuerDuty> {
        let role = effective_role(&env, &address);
        duties_of_role(&env, &role)
    }

    /// Returns the address that last approved `user`'s compliance, or `None`
    /// if `user` has never been approved. Pure read.
    ///
    /// This is the record `require_independent_approver` is evaluated against,
    /// and it is exposed so a reviewer can verify a four-eyes claim without
    /// replaying the event stream.
    pub fn get_compliance_approver(env: Env, user: Address) -> Option<Address> {
        get_compliance_approver(&env, &user)
    }

    /// Returns whether `caller` could issue to `recipient` under the current
    /// separation policy, and the precise reason when they could not.
    ///
    /// Pure read: no authorization, no writes, never reverts, callable while
    /// paused. The verdict comes from the same evaluation `mint_asset`
    /// enforces, so `allowed == false` guarantees a mint would revert with
    /// `error_code`.
    ///
    /// **Separation only.** This answers "may this key issue to this address",
    /// not "will this mint succeed": the recipient's compliance status, the
    /// supply and holding caps, the asset lifecycle, the pause, and the amount
    /// are all checked separately by `mint_asset`. Use
    /// `check_mint_restriction` for those.
    pub fn check_issuance_authority(
        env: Env,
        caller: Address,
        recipient: Address,
    ) -> IssuanceAuthorityCheck {
        check_issuance(&env, &caller, &recipient)
    }
}
