#![cfg(test)]

use crate::{config::ProtocolConfig, AegisContract, AegisContractClient, Error, Role};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup() -> (Env, AegisContractClient<'static>, Address, Address) {
    let env = Env::default();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    env.mock_all_auths();
    client.initialize(&admin);
    (env, client, admin, user1)
}

#[test]
fn test_config_initial_defaults() {
    let (_env, client, _, _) = setup();
    let config = client.get_protocol_config();
    assert_eq!(config.min_transfer_amount, 0);
    assert_eq!(config.max_batch_size, 100);
}

#[test]
fn test_propose_and_accept_config() {
    let (env, client, admin, _) = setup();
    env.mock_all_auths();

    let new_config = ProtocolConfig {
        min_transfer_amount: 100,
        max_batch_size: 50,
    };

    // 1. Propose
    client.propose_config(&admin, &new_config);

    // Verify candidate is set
    assert_eq!(
        client.get_pending_protocol_config(),
        Some(new_config.clone())
    );
    // Verify active is not changed yet
    assert_eq!(client.get_protocol_config().min_transfer_amount, 0);

    // 2. Accept
    client.accept_config(&admin);

    // Verify candidate is cleared
    assert_eq!(client.get_pending_protocol_config(), None);
    // Verify active is updated
    let active_config = client.get_protocol_config();
    assert_eq!(active_config.min_transfer_amount, 100);
    assert_eq!(active_config.max_batch_size, 50);
}

#[test]
fn test_cancel_config_proposal() {
    let (env, client, admin, _) = setup();
    env.mock_all_auths();

    let new_config = ProtocolConfig {
        min_transfer_amount: 100,
        max_batch_size: 50,
    };

    client.propose_config(&admin, &new_config);
    assert_eq!(client.get_pending_protocol_config(), Some(new_config));

    client.cancel_config_proposal(&admin);
    assert_eq!(client.get_pending_protocol_config(), None);
}

#[test]
fn test_unauthorized_config_changes() {
    let (env, client, admin, user1) = setup();
    env.mock_all_auths();

    let new_config = ProtocolConfig {
        min_transfer_amount: 100,
        max_batch_size: 50,
    };

    // user1 (no role) tries to propose
    let result = client.try_propose_config(&user1, &new_config);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));

    // user1 tries to cancel without proposal
    let result = client.try_cancel_config_proposal(&user1);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));

    // Set user1 to AssetManager and try again
    client.set_role(&admin, &user1, &Role::AssetManager);
    let result = client.try_propose_config(&user1, &new_config);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_malformed_config_rejection() {
    let (env, client, admin, _) = setup();
    env.mock_all_auths();

    let malformed_config_1 = ProtocolConfig {
        min_transfer_amount: -10, // Invalid: negative
        max_batch_size: 50,
    };

    let result = client.try_propose_config(&admin, &malformed_config_1);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));

    let malformed_config_2 = ProtocolConfig {
        min_transfer_amount: 100,
        max_batch_size: 0, // Invalid: 0 batch size would brick
    };

    let result = client.try_propose_config(&admin, &malformed_config_2);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn test_config_operations_blocked_when_paused() {
    let (env, client, admin, _) = setup();
    env.mock_all_auths();

    client.set_role(&admin, &admin, &Role::EmergencyOfficer);
    client.pause(&admin);

    let new_config = ProtocolConfig {
        min_transfer_amount: 100,
        max_batch_size: 50,
    };

    let result = client.try_propose_config(&admin, &new_config);
    assert_eq!(result, Err(Ok(Error::ContractPaused)));
}
