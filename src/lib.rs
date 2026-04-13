#![no_std]

pub mod asset;
pub mod compliance;
#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Whitelist(Address),
    Balance(Address),
    TotalSupply,
}

#[contract]
pub struct AegisContract;

#[contractimpl]
impl AegisContract {
    /// Initializes the contract with an admin. Can only be called once.
    pub fn initialize(env: Env, admin: Address) {
        assert!(
            !env.storage().instance().has(&DataKey::Admin),
            "Contract already initialized"
        );
        env.storage().instance().set(&DataKey::Admin, &admin);
    }
}