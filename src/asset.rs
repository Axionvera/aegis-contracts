use soroban_sdk::{contractimpl, Address, Env};

use crate::admin::{require_not_paused, require_role};
use crate::compliance;
use crate::{AegisContract, AegisContractArgs, AegisContractClient, DataKey, Error, Role};

#[contractimpl]
impl AegisContract {
    /// Mints new RWA tokens to a whitelisted address.
    /// Requires the AssetManager role or Admin.
    /// Blocked when the contract is paused.
    pub fn mint_asset(env: Env, admin: Address, to: Address, amount: i128) -> Result<(), Error> {
        require_not_paused(&env);
        admin.require_auth();
        require_role(&env, &admin, Role::AssetManager);
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        if !compliance::is_whitelisted(&env, &to) {
            return Err(Error::ReceiverNotWhitelisted);
        }

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

        Ok(())
    }

    /// Transfers tokens between two whitelisted addresses.
    /// Blocked when the contract is paused.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), Error> {
        require_not_paused(&env);
        from.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        if !compliance::is_whitelisted(&env, &from) {
            return Err(Error::SenderNotWhitelisted);
        }
        if !compliance::is_whitelisted(&env, &to) {
            return Err(Error::ReceiverNotWhitelisted);
        }

        let mut from_balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(from.clone()))
            .unwrap_or(0);
        if from_balance < amount {
            return Err(Error::InsufficientBalance);
        }

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

        Ok(())
    }

    /// Mocks the distribution of yield to current token holders.
    /// Requires the AssetManager role or Admin.
    /// Blocked when the contract is paused.
    pub fn distribute_yield(env: Env, admin: Address, amount: i128) -> Result<(), Error> {
        require_not_paused(&env);
        admin.require_auth();
        require_role(&env, &admin, Role::AssetManager);
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        // Mock implementation. Real-world Soroban implementation requires
        // snapshotting balances or utilizing a claim-based dividend pull pattern
        // rather than iterating over maps to avoid gas limits.
        // TODO: Implement scalable yield snapshot mechanism

        Ok(())
    }
}
