use soroban_sdk::{contractimpl, contracttype, Address, Env};

use crate::admin::{require_any_role, require_not_paused};
use crate::{AegisContract, AegisContractArgs, AegisContractClient, DataKey, Error, Role};

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

#[contractimpl]
impl AegisContract {
    /// Adds a user to the compliance whitelist.
    /// Requires the ComplianceOfficer role, EmergencyOfficer role, or Admin.
    /// Blocked when the contract is paused.
    pub fn whitelist_user(env: Env, admin: Address, user: Address) -> Result<(), Error> {
        require_not_paused(&env);
        admin.require_auth();
        require_any_role(
            &env,
            &admin,
            &[Role::ComplianceOfficer, Role::EmergencyOfficer],
        );

        // TODO: Implement batch whitelisting to save gas
        env.storage()
            .persistent()
            .set(&DataKey::Whitelist(user.clone()), &true);

        env.events().publish(
            ("user_whitelisted",),
            UserWhitelistedEvent {
                caller: admin,
                user,
            },
        );

        Ok(())
    }

    /// Removes a user from the compliance whitelist.
    /// Requires the ComplianceOfficer role, EmergencyOfficer role, or Admin.
    /// Blocked when the contract is paused.
    pub fn revoke_whitelist(env: Env, admin: Address, user: Address) -> Result<(), Error> {
        require_not_paused(&env);
        admin.require_auth();
        require_any_role(
            &env,
            &admin,
            &[Role::ComplianceOfficer, Role::EmergencyOfficer],
        );

        env.storage()
            .persistent()
            .remove(&DataKey::Whitelist(user.clone()));

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

/// Internal helper to check whitelist status across modules.
pub fn is_whitelisted(env: &Env, user: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Whitelist(user.clone()))
        .unwrap_or(false)
}
