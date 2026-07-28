use crate::{events, AegisContract, DataKey};
use crate::{AegisContractArgs, AegisContractClient};
use soroban_sdk::{contractimpl, Address, Env};

#[contractimpl]
impl AegisContract {
    /// Adds a user to the compliance whitelist. Only the admin can call this.
    pub fn whitelist_user(env: Env, admin: Address, user: Address) {
        admin.require_auth();
        let current_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        assert_eq!(
            admin, current_admin,
            "Unauthorized: Only admin can whitelist"
        );

        // TODO: Implement batch whitelisting to save gas
        env.storage()
            .persistent()
            .set(&DataKey::Whitelist(user.clone()), &true);

        events::user_whitelisted(&env, &admin, &user);
    }
}

/// Internal helper to check whitelist status across modules
pub fn is_whitelisted(env: &Env, user: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Whitelist(user.clone()))
        .unwrap_or(false)
}
