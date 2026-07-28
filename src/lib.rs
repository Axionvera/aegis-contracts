#![no_std]
pub mod admin;
pub mod asset;
pub mod capabilities;
pub mod compliance;
pub mod eligibility;
pub mod errors;
pub mod holding;
pub mod supply_cap;
#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

pub use errors::Error;

/// Role-based access control levels.
/// Admin is the supreme authority; other roles grant scoped privileges.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    /// No role assigned.
    None,
    /// Supreme authority. Set once at initialization; transferred only via
    /// the 2-step `transfer_admin` / `accept_admin` flow.
    Admin,
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
    ///
    /// Written only by the compliance lifecycle writer as a derived mirror of
    /// `ComplianceStatus(Address)`: present and `true` iff the address's
    /// lifecycle status is `Approved`. Never the source of truth — read
    /// `ComplianceStatus` instead. See `docs/compliance-lifecycle.md`.
    Whitelist(Address),
    /// Full compliance lifecycle status for an investor address. Absent means
    /// `ComplianceStatus::Unknown` (the safe default: nothing is permitted).
    ComplianceStatus(Address),
    /// Token balance for an address.
    Balance(Address),
    /// Global total supply counter.
    TotalSupply,
    /// Whether the contract is paused. If `true`, all state-changing
    /// operations (minting, transfers, compliance) are blocked.
    Paused,
    /// The currently active global supply cap. A value of `0` means
    /// "no cap enforced" (unbounded minting, subject to whitelist).
    SupplyCap,
    /// The pending (proposed) supply cap awaiting 2-step acceptance.
    SupplyCapCandidate,
    /// The currently active per-investor holding cap. A value of `0` means
    /// "no holding restriction" (any whitelisted balance is allowed).
    HoldingCap,
    /// The pending (proposed) holding cap awaiting 2-step acceptance.
    HoldingCapCandidate,
    /// Current lifecycle status for the issued asset.
    AssetStatus,
    /// Display name for the issued asset.
    AssetName,
    /// Ticker symbol for the issued asset.
    AssetSymbol,
    /// Optional metadata URI for off-chain asset details.
    AssetMetadataUri,
}

#[contract]
pub struct AegisContract;

#[contractimpl]
impl AegisContract {
    /// Initializes the contract with an admin. Can only be called once.
    /// The initial admin is assigned the Admin role implicitly.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        // Grant the Admin role to the initial admin.
        env.storage()
            .persistent()
            .set(&DataKey::Role(admin), &Role::Admin);
        Ok(())
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
