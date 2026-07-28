#![no_std]

pub mod admin;
pub mod asset;
pub mod compliance;
#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

/// Role-based access control levels.
/// Admin is the supreme authority; other roles grant scoped privileges.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    /// No role assigned.
    None,
    /// Can manage the compliance whitelist.
    ComplianceOfficer,
    /// Can mint assets and distribute yield.
    AssetManager,
    /// Combined compliance + asset privileges for operational flexibility.
    EmergencyOfficer,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// The supreme admin address (set once at initialization).
    Admin,
    /// Candidate for the next admin during a 2-step transfer.
    AdminCandidate,
    /// The role assigned to a specific address.
    Role(Address),
    /// Legacy whitelist flag (kept for backwards compatibility).
    Whitelist(Address),
    /// Token balance for an address.
    Balance(Address),
    /// Global total supply counter.
    TotalSupply,
    /// Whether the contract is paused. If `true`, all state-changing
    /// operations (minting, transfers, compliance) are blocked.
    Paused,
}

#[contract]
pub struct AegisContract;

#[contractimpl]
impl AegisContract {
    /// Initializes the contract with an admin. Can only be called once.
    /// The initial admin is assigned the Admin role implicitly.
    pub fn initialize(env: Env, admin: Address) {
        assert!(
            !env.storage().instance().has(&DataKey::Admin),
            "Contract already initialized"
        );
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        // Grant the Admin role to the initial admin.
        env.storage()
            .persistent()
            .set(&DataKey::Role(admin), &Role::Admin);
    }

    /// Returns the token balance for an address.
    pub fn get_balance_of(env: Env, address: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(address))
            .unwrap_or(0)
    }

    /// Returns the global total supply.
    pub fn get_total_supply(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0)
    }

    /// Returns whether an address is on the compliance whitelist.
    pub fn is_whitelisted(env: Env, user: Address) -> bool {
        compliance::is_whitelisted(&env, &user)
    }
}
