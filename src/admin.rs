use soroban_sdk::{contractimpl, contracttype, Address, Env};

use crate::{AegisContract, DataKey, Role};

// ─── Pause helpers ────────────────────────────────────────────────────────────

/// Returns `true` if the contract is currently paused.
pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

/// Asserts that the contract is **not** paused. Reverts with a descriptive
/// message if the contract is paused. Call this at the top of every
/// state-changing operation that should be blocked during a pause.
pub fn require_not_paused(env: &Env) {
    assert!(!is_paused(env), "Contract is paused");
}

// ─── Events ───────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct RoleAssignedEvent {
    pub admin: Address,
    pub target: Address,
    pub role: Role,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct RoleRevokedEvent {
    pub admin: Address,
    pub target: Address,
    pub role: Role,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct AdminTransferInitiatedEvent {
    pub current_admin: Address,
    pub candidate: Address,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct AdminTransferredEvent {
    pub previous_admin: Address,
    pub new_admin: Address,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ContractPausedEvent {
    pub admin: Address,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ContractUnpausedEvent {
    pub admin: Address,
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Returns the current supreme admin address. Panics if not initialized.
pub fn get_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .expect("Admin not initialized")
}

/// Returns the role assigned to an address, or Role::None if unassigned.
pub fn get_role(env: &Env, address: &Address) -> Role {
    env.storage()
        .persistent()
        .get(&DataKey::Role(address.clone()))
        .unwrap_or(Role::None)
}

/// Asserts that `caller` has the required `role` or is the supreme admin.
/// Reverts with a descriptive message on failure.
pub fn require_role(env: &Env, caller: &Address, required: Role) {
    let role = get_role(env, caller);
    let admin = get_admin(env);

    // The supreme admin bypasses all role checks.
    if *caller == admin {
        return;
    }

    assert!(role == required, "Unauthorized: required role not held");
}

// ─── Public API ───────────────────────────────────────────────────────────────

#[contractimpl]
impl AegisContract {
    /// Assigns a role to `target`. Only the supreme admin can call this.
    /// Blocked when the contract is paused.
    pub fn set_role(env: Env, admin: Address, target: Address, role: Role) {
        require_not_paused(&env);
        admin.require_auth();
        assert_eq!(
            admin,
            get_admin(&env),
            "Unauthorized: only admin can assign roles"
        );

        // Prevent assigning the Admin role to another address — use
        // transfer_admin for a safe 2-step handoff instead.
        assert_ne!(
            role,
            Role::Admin,
            "Cannot assign Admin role via set_role; use transfer_admin"
        );

        env.storage()
            .persistent()
            .set(&DataKey::Role(target.clone()), &role);

        env.events().publish(
            ("role_assigned",),
            RoleAssignedEvent {
                admin,
                target,
                role,
            },
        );
    }

    /// Revokes the role from `target`, setting it to Role::None.
    /// Only the supreme admin can call this.
    /// Blocked when the contract is paused.
    pub fn remove_role(env: Env, admin: Address, target: Address) {
        require_not_paused(&env);
        admin.require_auth();
        assert_eq!(
            admin,
            get_admin(&env),
            "Unauthorized: only admin can revoke roles"
        );

        let previous_role = get_role(&env, &target);
        assert_ne!(previous_role, Role::None, "Target has no role to revoke");

        env.storage()
            .persistent()
            .set(&DataKey::Role(target.clone()), &Role::None);

        env.events().publish(
            ("role_revoked",),
            RoleRevokedEvent {
                admin,
                target,
                role: previous_role,
            },
        );
    }

    /// Returns the role assigned to `address`.
    pub fn get_role_of(env: Env, address: Address) -> Role {
        get_role(&env, &address)
    }

    /// Initiates a 2-step admin transfer. Sets `candidate` as the pending new
    /// admin. Only the current admin can call this.
    /// Blocked when the contract is paused.
    pub fn transfer_admin(env: Env, admin: Address, candidate: Address) {
        require_not_paused(&env);
        admin.require_auth();
        assert_eq!(
            admin,
            get_admin(&env),
            "Unauthorized: only admin can transfer admin"
        );

        env.storage()
            .instance()
            .set(&DataKey::AdminCandidate, &candidate);

        env.events().publish(
            ("admin_transfer_initiated",),
            AdminTransferInitiatedEvent {
                current_admin: admin,
                candidate,
            },
        );
    }

    /// Completes a 2-step admin transfer. The `candidate` must call this to
    /// accept the role. Only the pending candidate can call this.
    /// Blocked when the contract is paused.
    pub fn accept_admin(env: Env, candidate: Address) {
        require_not_paused(&env);
        candidate.require_auth();

        let stored_candidate: Address = env
            .storage()
            .instance()
            .get(&DataKey::AdminCandidate)
            .expect("No pending admin transfer");

        assert_eq!(
            candidate, stored_candidate,
            "Caller is not the pending admin candidate"
        );

        let previous_admin = get_admin(&env);

        // Clear the candidate slot.
        env.storage().instance().remove(&DataKey::AdminCandidate);

        // Set the new admin.
        env.storage().instance().set(&DataKey::Admin, &candidate);

        // Transfer the Admin role: revoke from old, grant to new.
        env.storage()
            .persistent()
            .set(&DataKey::Role(previous_admin.clone()), &Role::None);
        env.storage()
            .persistent()
            .set(&DataKey::Role(candidate.clone()), &Role::Admin);

        env.events().publish(
            ("admin_transferred",),
            AdminTransferredEvent {
                previous_admin,
                new_admin: candidate,
            },
        );
    }

    /// The current admin can renounce their own admin role. This is an
    /// irreversible action — use with caution.
    /// Blocked when the contract is paused.
    pub fn renounce_admin(env: Env, admin: Address) {
        require_not_paused(&env);
        admin.require_auth();
        assert_eq!(
            admin,
            get_admin(&env),
            "Unauthorized: only admin can renounce"
        );

        env.storage().instance().remove(&DataKey::Admin);
        env.storage()
            .persistent()
            .set(&DataKey::Role(admin.clone()), &Role::None);

        env.events().publish(
            ("admin_renounced",),
            AdminTransferredEvent {
                previous_admin: admin,
                new_admin: admin, // Self-renounced
            },
        );
    }

    /// Returns whether the contract is currently paused.
    pub fn is_paused(env: Env) -> bool {
        is_paused(&env)
    }

    /// Pauses the contract. When paused, all state-changing operations
    /// (minting, transfers, compliance changes) are blocked. Read functions
    /// remain available.
    ///
    /// Only the admin or an EmergencyOfficer can pause the contract.
    pub fn pause(env: Env, caller: Address) {
        caller.require_auth();
        require_role(&env, &caller, Role::EmergencyOfficer);

        assert!(!is_paused(&env), "Contract is already paused");

        env.storage().instance().set(&DataKey::Paused, &true);

        env.events()
            .publish(("contract_paused",), ContractPausedEvent { admin: caller });
    }

    /// Unpauses the contract, restoring normal operations.
    ///
    /// Only the admin can unpause — this ensures that a compromised
    /// EmergencyOfficer cannot unpause after a legitimate admin-initiated pause.
    pub fn unpause(env: Env, caller: Address) {
        caller.require_auth();
        assert_eq!(
            caller,
            get_admin(&env),
            "Unauthorized: only admin can unpause"
        );

        assert!(is_paused(&env), "Contract is not paused");

        env.storage().instance().set(&DataKey::Paused, &false);

        env.events().publish(
            ("contract_unpaused",),
            ContractUnpausedEvent { admin: caller },
        );
    }
}
