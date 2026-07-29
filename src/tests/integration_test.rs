use soroban_sdk::{Env, Address};
use aegis_contracts::{AegisContract, AegisContractClient};

#[test]
fn test_integration_flow() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    client.initialize(&admin);
}
