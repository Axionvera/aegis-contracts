#![allow(deprecated)]

use soroban_sdk::{contractimpl, contracttype, Env, Address};
use crate::{admin::{get_admin, require_not_paused}, AegisContract, AegisContractClient, AegisContractArgs, DataKey, Error};

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolConfig {
    /// The minimum token amount allowed in a single transfer. 
    /// 0 means no minimum is enforced.
    pub min_transfer_amount: i128,
    
    /// The maximum number of operations allowed in a single batch.
    /// Useful for enforcing gas/compute limits in batched compliance or transfer functions.
    pub max_batch_size: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ConfigProposedEvent {
    pub admin: Address,
    pub proposed_config: ProtocolConfig,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ConfigAmendedEvent {
    pub admin: Address,
    pub new_config: ProtocolConfig,
}

/// Returns the currently active protocol configuration.
/// If no configuration has been set, returns a safe default.
pub fn get_config(env: &Env) -> ProtocolConfig {
    env.storage()
        .instance()
        .get(&DataKey::ProtocolConfig)
        .unwrap_or(ProtocolConfig {
            min_transfer_amount: 0,
            max_batch_size: 100, // Safe default to prevent infinite loops in batch operations
        })
}

/// Returns the currently pending proposed configuration, if any.
pub fn get_pending_config(env: &Env) -> Option<ProtocolConfig> {
    env.storage().instance().get(&DataKey::ProtocolConfigCandidate)
}

#[contractimpl]
impl AegisContract {
    /// Proposes a new global protocol configuration.
    /// Uses a 2-step process to prevent accidental bricking due to malformed parameters.
    /// Only the supreme admin can call this.
    /// Blocked when the contract is paused.
    pub fn propose_config(
        env: Env,
        admin: Address,
        proposed_config: ProtocolConfig,
    ) -> Result<(), Error> {
        require_not_paused(&env);
        admin.require_auth();
        if admin != get_admin(&env) {
            return Err(Error::Unauthorized);
        }

        // Validate the configuration to prevent bricking
        if proposed_config.min_transfer_amount < 0 {
            return Err(Error::InvalidAmount);
        }
        if proposed_config.max_batch_size == 0 {
            // A max batch size of 0 would brick batched operations entirely
            return Err(Error::InvalidAmount);
        }

        env.storage()
            .instance()
            .set(&DataKey::ProtocolConfigCandidate, &proposed_config);

        env.events().publish(
            ("config_proposed",),
            ConfigProposedEvent {
                admin,
                proposed_config,
            },
        );

        Ok(())
    }

    /// Accepts the pending proposed protocol configuration, making it active.
    /// Only the supreme admin can call this.
    /// Blocked when the contract is paused.
    pub fn accept_config(env: Env, admin: Address) -> Result<(), Error> {
        require_not_paused(&env);
        admin.require_auth();
        if admin != get_admin(&env) {
            return Err(Error::Unauthorized);
        }

        let pending_config: ProtocolConfig =
            match env.storage().instance().get(&DataKey::ProtocolConfigCandidate) {
                Some(config) => config,
                None => return Err(Error::NoPendingAdminTransfer), // Reusing error, ideally would add NoPendingConfig
            };

        // Clear the candidate
        env.storage()
            .instance()
            .remove(&DataKey::ProtocolConfigCandidate);

        // Apply the new config
        env.storage()
            .instance()
            .set(&DataKey::ProtocolConfig, &pending_config);

        env.events().publish(
            ("config_amended",),
            ConfigAmendedEvent {
                admin,
                new_config: pending_config,
            },
        );

        Ok(())
    }

    /// Cancels a pending protocol configuration proposal.
    /// Only the supreme admin can call this.
    /// Blocked when the contract is paused.
    pub fn cancel_config_proposal(env: Env, admin: Address) -> Result<(), Error> {
        require_not_paused(&env);
        admin.require_auth();
        if admin != get_admin(&env) {
            return Err(Error::Unauthorized);
        }

        if !env.storage().instance().has(&DataKey::ProtocolConfigCandidate) {
            return Err(Error::NoPendingAdminTransfer); // Using existing error
        }

        env.storage()
            .instance()
            .remove(&DataKey::ProtocolConfigCandidate);

        Ok(())
    }

    /// Returns the currently active protocol configuration.
    pub fn get_protocol_config(env: Env) -> ProtocolConfig {
        get_config(&env)
    }

    /// Returns the currently pending protocol configuration proposal.
    pub fn get_pending_protocol_config(env: Env) -> Option<ProtocolConfig> {
        get_pending_config(&env)
    }
}
