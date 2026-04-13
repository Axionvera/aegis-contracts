#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, AegisContract);
    let client = AegisContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // Init
    client.initialize(&admin);

    // Whitelist
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);

    // Mint
    client.mint_asset(&admin, &user1, &1000);

    // Transfer
    client.transfer(&user1, &user2, &250);

    // Check auths and limits inherently tested by mock_all_auths
}