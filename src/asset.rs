use soroban_sdk::{contractimpl, Address, Env};

use crate::admin::{get_admin, require_role};
use crate::compliance;
use crate::{AegisContract, DataKey, Role};

#[contractimpl]
impl AegisContract {
    /// Mints new RWA tokens to a whitelisted address.
    /// Requires the AssetManager role or Admin.
    pub fn mint_asset(env: Env, admin: Address, to: Address, amount: i128) {
        admin.require_auth();
        require_role(&env, &admin, Role::AssetManager);
        assert!(amount > 0, "Amount must be positive");

        assert!(
            compliance::is_whitelisted(&env, &to),
            "Receiver is not whitelisted"
        );

        let mut balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);
        balance += amount;
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to), &balance);

        let mut supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        supply += amount;
        env.storage().instance().set(&DataKey::TotalSupply, &supply);
    }

    /// Transfers tokens between two whitelisted addresses.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        assert!(amount > 0, "Amount must be positive");

        assert!(
            compliance::is_whitelisted(&env, &from),
            "Sender is not whitelisted"
        );
        assert!(
            compliance::is_whitelisted(&env, &to),
            "Receiver is not whitelisted"
        );

        let mut from_balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(from.clone()))
            .unwrap_or(0);
        assert!(from_balance >= amount, "Insufficient balance");

        // TODO: Implement fee deduction on transfer
        from_balance -= amount;
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from), &from_balance);

        let mut to_balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);
        to_balance += amount;
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to), &to_balance);
    }

    /// Mocks the distribution of yield to current token holders.
    /// Requires the AssetManager role or Admin.
    pub fn distribute_yield(env: Env, admin: Address, amount: i128) {
        admin.require_auth();
        require_role(&env, &admin, Role::AssetManager);
        assert!(amount > 0, "Amount must be positive");

        // Mock implementation. Real-world Soroban implementation requires
        // snapshotting balances or utilizing a claim-based dividend pull pattern
        // rather than iterating over maps to avoid gas limits.
        // TODO: Implement scalable yield snapshot mechanism
    }
}
