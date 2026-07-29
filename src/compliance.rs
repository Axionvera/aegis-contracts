use crate::{events, AegisContract, DataKey};
use crate::{AegisContractArgs, AegisContractClient};
use soroban_sdk::{contractimpl, Address, Env};

#[contractimpl]
impl AegisContract {
    /// Adds a user to the compliance whitelist. Only the admin can call this.
    /// Clears any prior revocation flag, allowing re-onboarding after compliance review.
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
        // Clear revocation if it was previously set - re-whitelisting restores compliance
        env.storage()
            .persistent()
            .set(&DataKey::Revoked(user.clone()), &false);

        events::user_whitelisted(&env, &admin, &user);
    }

    /// Revokes a previously whitelisted investor.
    /// Only the admin can call this.
    ///
    /// Policy:
    /// - Sets `Whitelist(user) = false` and `Revoked(user) = true`
    /// - Revoked users are fully frozen: cannot receive (mint/transfer-in) nor send (transfer-out)
    /// - Existing balance is retained for record-keeping and potential forced redemption
    ///   via off-chain process or future re-whitelisting. Balance is NOT burned.
    /// - Emits `wl_rev` event for off-chain monitoring.
    pub fn revoke_user(env: Env, admin: Address, user: Address) {
        admin.require_auth();
        let current_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        assert_eq!(admin, current_admin, "Unauthorized: Only admin can revoke");

        // Ensure user was at least once whitelisted or already tracked? Allow revoking any address
        // to support pre-emptive blocklisting, but typically should have been whitelisted.
        env.storage()
            .persistent()
            .set(&DataKey::Whitelist(user.clone()), &false);
        env.storage()
            .persistent()
            .set(&DataKey::Revoked(user.clone()), &true);

        events::user_revoked(&env, &admin, &user);
    }

    /// Unrevokes a user without automatically re-whitelisting.
    /// Admin must call `whitelist_user` afterwards to restore transfer/mint privileges.
    /// This exists for cases where you want to clear the revoked flag but require a separate
    /// compliance approval step.
    /// Alternatively, `whitelist_user` itself clears revocation.
    pub fn unrevoke_user(env: Env, admin: Address, user: Address) {
        admin.require_auth();
        let current_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        assert_eq!(admin, current_admin, "Unauthorized: Only admin can unrevoke");

        env.storage()
            .persistent()
            .set(&DataKey::Revoked(user.clone()), &false);

        // Re-use whitelist_add event? Better to emit whitelisted again only when whitelisting.
        // For audit trail, we emit a wl_add-like event with a special marker? Simpler: emit
        // whitelist add if we want, but spec asks for revocation event only. We'll emit
        // whitelist cleared as revocation removal via a whitelist event? To keep event surface
        // minimal, we emit user_whitelisted if they become whitelisted, otherwise no event here
        // is confusing. Let's emit revoked with false? Simpler: do not emit event here, or emit
        // wl_add if re-whitelisted via whitelist_user.
        // For now, emit a whitelisted event only when whitelist_user is called; unrevoke is silent
        // except storage change. Alternatively, emit revoked clear as wl_rev with admin but we need
        // distinct event. For simplicity, we publish a wl_add event with admin as marker of clearance
        // is not ideal. We'll introduce no event for unrevoke and recommend using whitelist_user.
        // However to keep audit, we emit user_whitelisted if needed? Actually we will emit a
        // dedicated event via `user_unrevoked` is not defined yet - we choose to emit whitelist event
        // to signal compliance restoration path is via whitelist_user. So this function is internal
        // utility and does not emit.
    }

    /// Returns true if user is whitelisted and not revoked.
    pub fn is_whitelisted_check(env: Env, user: Address) -> bool {
        is_whitelisted(&env, &user)
    }

    /// Returns true if user is revoked.
    pub fn is_revoked_check(env: Env, user: Address) -> bool {
        is_revoked(&env, &user)
    }

    /// Returns detailed compliance status for off-chain querying.
    pub fn compliance_status(env: Env, user: Address) -> (bool, bool) {
        (is_whitelisted(&env, &user), is_revoked(&env, &user))
    }
}

/// Internal helper to check whitelist status across modules
/// Whitelisted means: Whitelist flag true AND not revoked.
pub fn is_whitelisted(env: &Env, user: &Address) -> bool {
    let whitelisted: bool = env
        .storage()
        .persistent()
        .get(&DataKey::Whitelist(user.clone()))
        .unwrap_or(false);
    if !whitelisted {
        return false;
    }
    // Even if whitelist flag is true, revocation overrides
    !is_revoked(env, user)
}

/// Internal helper to check revocation status
pub fn is_revoked(env: &Env, user: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Revoked(user.clone()))
        .unwrap_or(false)
}
