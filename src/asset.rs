use crate::{compliance, events, AegisContract, DataKey};
use crate::{AegisContractArgs, AegisContractClient};
use soroban_sdk::{contractimpl, Address, Env};

#[contractimpl]
impl AegisContract {
    /// Mints new RWA tokens to a whitelisted address.
    pub fn mint_asset(env: Env, admin: Address, to: Address, amount: i128) {
        admin.require_auth();
        let current_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        assert_eq!(admin, current_admin, "Unauthorized: Only admin can mint");
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
            .set(&DataKey::Balance(to.clone()), &balance);

        let mut supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        supply += amount;
        env.storage().instance().set(&DataKey::TotalSupply, &supply);

        events::asset_minted(&env, &to, amount, balance, supply);
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
            .set(&DataKey::Balance(from.clone()), &from_balance);

        let mut to_balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);
        to_balance += amount;
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &to_balance);

        events::asset_transferred(&env, &from, &to, amount);
    }

    /// Mocks the distribution of yield to current token holders.
    pub fn distribute_yield(env: Env, admin: Address, amount: i128) {
        admin.require_auth();
        let current_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        assert_eq!(admin, current_admin, "Unauthorized");
        assert!(amount > 0, "Amount must be positive");

        // Mock implementation. Real-world Soroban implementation requires
        // snapshotting balances or utilizing a claim-based dividend pull pattern
        // rather than iterating over maps to avoid gas limits.
        // TODO: Implement scalable yield snapshot mechanism
        let supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);

        events::yield_distributed(&env, &admin, amount, supply);
    }
}
