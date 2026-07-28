use soroban_sdk::{contractimpl, Address, Env};

use crate::admin::{require_not_paused, require_role};
use crate::{AegisContract, DataKey, Role};

#[contractimpl]
impl AegisContract {
    /// Adds a user to the compliance whitelist.
    /// Requires the ComplianceOfficer role, EmergencyOfficer role, or Admin.
    /// Blocked when the contract is paused.
    pub fn whitelist_user(env: Env, admin: Address, user: Address) {
        require_not_paused(&env);
        admin.require_auth();
        require_role(&env, &admin, Role::ComplianceOfficer);

        // TODO: Implement batch whitelisting to save gas
        env.storage()
            .persistent()
            .set(&DataKey::Whitelist(user), &true);

        // TODO: Add events for compliance tracking
    }

    /// Removes a user from the compliance whitelist.
    /// Requires the ComplianceOfficer role, EmergencyOfficer role, or Admin.
    /// Blocked when the contract is paused.
    pub fn revoke_whitelist(env: Env, admin: Address, user: Address) {
        require_not_paused(&env);
        admin.require_auth();
        require_role(&env, &admin, Role::ComplianceOfficer);

        env.storage().persistent().remove(&DataKey::Whitelist(user));

        // TODO: Add events for compliance tracking
    }
}

/// Internal helper to check whitelist status across modules.
pub fn is_whitelisted(env: &Env, user: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Whitelist(user.clone()))
        .unwrap_or(false)
}
