#![cfg(test)]

// The crate is `#![no_std]`, but the test harness (and soroban-sdk's testutils)
// link against std, so bring it into scope for the test module only.
extern crate std;

use super::*;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    xdr::{ContractEventBody, ScVal, VecM},
    Address, Env, IntoVal, TryFromVal, Val,
};

/// Convert any host value into its XDR `ScVal` form for comparison.
fn sc(env: &Env, value: Val) -> ScVal {
    ScVal::try_from_val(env, &value).expect("value convertible to ScVal")
}

/// Build the expected topic vector in the same XDR form the host emits.
fn sc_topics(env: &Env, topics: soroban_sdk::Vec<Val>) -> VecM<ScVal> {
    let mut out = std::vec::Vec::new();
    for topic in topics.iter() {
        out.push(sc(env, topic));
    }
    out.try_into().expect("topic vec within XDR limits")
}

/// Events published by our contract during the **most recent invocation**,
/// in emission order, as (topics, data).
///
/// Note: `Env::events().all()` is scoped to the last contract invocation in
/// soroban-sdk v26 - it is not a cumulative log across calls. Tests therefore
/// assert per-call, and `collect_all` below accumulates when a full lifecycle
/// view is needed.
type EventPair = (VecM<ScVal>, ScVal);

fn contract_events(env: &Env, contract_id: &Address) -> std::vec::Vec<EventPair> {
    env.events()
        .all()
        .filter_by_contract(contract_id)
        .events()
        .iter()
        .map(|event| match &event.body {
            ContractEventBody::V0(v0) => (v0.topics.clone(), v0.data.clone()),
        })
        .collect()
}

#[test]
fn test_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(AegisContract, ());
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

#[test]
fn test_initialize_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let events = contract_events(&env, &contract_id);
    assert_eq!(events.len(), 1, "initialize must publish exactly one event");

    let (topics, data) = &events[0];
    let expected: soroban_sdk::Vec<Val> =
        (symbol_short!("aegis"), symbol_short!("init")).into_val(&env);
    assert_eq!(topics, &sc_topics(&env, expected));
    assert_eq!(data, &sc(&env, admin.into_val(&env)));
}

#[test]
fn test_whitelist_emits_compliance_event() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);
    client.whitelist_user(&admin, &user);

    // Scoped to the whitelist_user invocation.
    let events = contract_events(&env, &contract_id);
    assert_eq!(
        events.len(),
        1,
        "whitelist_user must publish exactly one event"
    );

    let (topics, data) = &events[0];
    let expected: soroban_sdk::Vec<Val> =
        (symbol_short!("aegis"), symbol_short!("wl_add"), user).into_val(&env);
    assert_eq!(topics, &sc_topics(&env, expected));
    assert_eq!(data, &sc(&env, admin.into_val(&env)));
}

#[test]
fn test_mint_emits_event_with_balance_and_supply() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);
    client.whitelist_user(&admin, &user);
    client.mint_asset(&admin, &user, &1000);
    client.mint_asset(&admin, &user, &500);

    // Scoped to the second mint_asset invocation.
    let events = contract_events(&env, &contract_id);
    assert_eq!(events.len(), 1, "mint_asset must publish exactly one event");

    let (topics, data) = &events[0];
    let expected: soroban_sdk::Vec<Val> =
        (symbol_short!("aegis"), symbol_short!("mint"), user).into_val(&env);
    assert_eq!(topics, &sc_topics(&env, expected));

    // Second mint: amount=500, running balance=1500, total supply=1500
    let expected_data: Val = (500i128, 1500i128, 1500i128).into_val(&env);
    assert_eq!(data, &sc(&env, expected_data));
}

#[test]
fn test_transfer_emits_event_with_both_parties() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.initialize(&admin);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&admin, &user1, &1000);
    client.transfer(&user1, &user2, &250);

    let events = contract_events(&env, &contract_id);
    let (topics, data) = events.last().expect("transfer event published");

    let expected: soroban_sdk::Vec<Val> = (
        symbol_short!("aegis"),
        symbol_short!("transfer"),
        user1,
        user2,
    )
        .into_val(&env);
    assert_eq!(topics, &sc_topics(&env, expected));
    assert_eq!(data, &sc(&env, 250i128.into_val(&env)));
}

#[test]
fn test_distribute_yield_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);
    client.whitelist_user(&admin, &user);
    client.mint_asset(&admin, &user, &1000);
    client.distribute_yield(&admin, &42);

    let events = contract_events(&env, &contract_id);
    let (topics, data) = events.last().expect("yield event published");

    let expected: soroban_sdk::Vec<Val> =
        (symbol_short!("aegis"), symbol_short!("yield")).into_val(&env);
    assert_eq!(topics, &sc_topics(&env, expected));

    let expected_data: Val = (admin, 42i128, 1000i128).into_val(&env);
    assert_eq!(data, &sc(&env, expected_data));
}

#[test]
fn test_every_state_change_is_observable() {
    // A monitoring service can only stream what the contract publishes.
    // This asserts every state mutation yields exactly one namespaced event.
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // Accumulate per-invocation events into a full lifecycle view.
    let mut collected: std::vec::Vec<EventPair> = std::vec::Vec::new();
    let mut record = |env: &Env| collected.extend(contract_events(env, &contract_id));

    client.initialize(&admin);
    record(&env);
    client.whitelist_user(&admin, &user1);
    record(&env);
    client.whitelist_user(&admin, &user2);
    record(&env);
    client.mint_asset(&admin, &user1, &1000);
    record(&env);
    client.transfer(&user1, &user2, &250);
    record(&env);
    client.distribute_yield(&admin, &10);
    record(&env);

    assert_eq!(
        collected.len(),
        6,
        "expected init + 2 whitelist + mint + transfer + yield"
    );

    // Every Aegis event must be namespaced so off-chain filters can pin topic 0.
    let namespace = sc(&env, symbol_short!("aegis").into_val(&env));
    let expected_actions = [
        symbol_short!("init"),
        symbol_short!("wl_add"),
        symbol_short!("wl_add"),
        symbol_short!("mint"),
        symbol_short!("transfer"),
        symbol_short!("yield"),
    ];

    for (index, (topics, _)) in collected.iter().enumerate() {
        let topic0 = topics.first().expect("event must carry a namespace topic");
        assert_eq!(
            topic0, &namespace,
            "every event must start with the `aegis` namespace topic"
        );
        let topic1 = topics.get(1).expect("event must carry an action topic");
        assert_eq!(
            topic1,
            &sc(&env, expected_actions[index].into_val(&env)),
            "action topic mismatch at position {}",
            index
        );
    }
}

#[test]
#[should_panic(expected = "Receiver is not whitelisted")]
fn test_mint_to_non_whitelisted_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.initialize(&admin);
    client.mint_asset(&admin, &stranger, &100);
}

#[test]
#[should_panic(expected = "Insufficient balance")]
fn test_transfer_insufficient_balance_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.initialize(&admin);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&admin, &user1, &100);
    client.transfer(&user1, &user2, &500);
}

#[test]
fn test_revoke_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);
    client.whitelist_user(&admin, &user);
    client.revoke_user(&admin, &user);

    let events = contract_events(&env, &contract_id);
    assert_eq!(events.len(), 1, "revoke_user must publish exactly one event");

    let (topics, data) = &events[0];
    let expected: soroban_sdk::Vec<Val> =
        (symbol_short!("aegis"), symbol_short!("wl_rev"), user).into_val(&env);
    assert_eq!(topics, &sc_topics(&env, expected));
    assert_eq!(data, &sc(&env, admin.into_val(&env)));
}

#[test]
#[should_panic(expected = "Receiver is revoked")]
fn test_revoked_cannot_receive_mint() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);
    client.whitelist_user(&admin, &user);
    client.mint_asset(&admin, &user, &1000);
    client.revoke_user(&admin, &user);
    // Should fail: revoked recipient cannot receive new restricted tokens
    client.mint_asset(&admin, &user, &100);
}

#[test]
#[should_panic(expected = "Receiver is revoked")]
fn test_revoked_cannot_receive_transfer() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.initialize(&admin);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&admin, &user1, &1000);
    client.revoke_user(&admin, &user2);
    // user2 is revoked, cannot receive
    client.transfer(&user1, &user2, &100);
}

#[test]
#[should_panic(expected = "Sender is revoked")]
fn test_revoked_cannot_send_transfer_fully_blocked() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.initialize(&admin);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&admin, &user1, &1000);
    client.revoke_user(&admin, &user1);
    // Fully blocked policy: revoked sender cannot transfer out
    client.transfer(&user1, &user2, &100);
}

#[test]
fn test_revoked_retains_balance_but_frozen() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);
    client.whitelist_user(&admin, &user);
    client.mint_asset(&admin, &user, &1000);
    client.revoke_user(&admin, &user);

    // Balance should still exist (not burned) but frozen
    let status: (bool, bool) = client.compliance_status(&user);
    assert_eq!(status.0, false, "whitelist should be false after revocation");
    assert_eq!(status.1, true, "revoked flag should be true");

    // Check whitelist helper returns false
    assert_eq!(client.is_whitelisted_check(&user), false);
    assert_eq!(client.is_revoked_check(&user), true);
}

#[test]
fn test_rewhitelist_clears_revocation() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.initialize(&admin);
    client.whitelist_user(&admin, &user);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&admin, &user, &1000);
    client.revoke_user(&admin, &user);

    // Re-whitelist after compliance review
    client.whitelist_user(&admin, &user);

    assert_eq!(client.is_revoked_check(&user), false);
    assert_eq!(client.is_whitelisted_check(&user), true);

    // Should now be able to receive and send again
    client.mint_asset(&admin, &user, &500);
    client.transfer(&user, &user2, &200);
}

#[test]
fn test_revocation_lifecycle_observable() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);

    let mut collected: std::vec::Vec<EventPair> = std::vec::Vec::new();
    let mut record = |env: &Env| collected.extend(contract_events(env, &contract_id));

    client.initialize(&admin);
    record(&env);
    client.whitelist_user(&admin, &user1);
    record(&env);
    client.mint_asset(&admin, &user1, &1000);
    record(&env);
    client.revoke_user(&admin, &user1);
    record(&env);

    assert_eq!(collected.len(), 4, "expected init + whitelist + mint + revoke");

    let namespace = sc(&env, symbol_short!("aegis").into_val(&env));
    let expected_actions = [
        symbol_short!("init"),
        symbol_short!("wl_add"),
        symbol_short!("mint"),
        symbol_short!("wl_rev"),
    ];

    for (idx, (topics, _)) in collected.iter().enumerate() {
        let topic0 = topics.first().expect("namespace");
        assert_eq!(topic0, &namespace);
        let topic1 = topics.get(1).expect("action");
        assert_eq!(
            topic1,
            &sc(&env, expected_actions[idx].into_val(&env)),
            "action mismatch {}",
            idx
        );
    }
}

#[test]
#[should_panic(expected = "Receiver is not whitelisted")]
fn test_non_whitelisted_still_blocked_after_unrevoke_without_whitelist() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(&admin);
    // Never whitelisted, but try unrevoke then mint - should still fail whitelist
    client.unrevoke_user(&admin, &user);
    client.mint_asset(&admin, &user, &100);
}

/// Dumps the real, host-produced XDR for every Aegis event as base64 so the
/// off-chain monitoring decoder can be verified against genuine contract
/// output. Ignored by default; run with:
///   cargo test dump_event_xdr -- --ignored --nocapture
#[test]
#[ignore]
fn dump_event_xdr() {
    use soroban_sdk::xdr::{Limits, WriteXdr};

    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AegisContract, ());
    let client = AegisContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    let dump = |env: &Env, label: &str| {
        for (topics, data) in contract_events(env, &contract_id) {
            let topic_b64: std::vec::Vec<std::string::String> = topics
                .iter()
                .map(|t| t.to_xdr_base64(Limits::none()).unwrap())
                .collect();
            std::println!(
                "XDRDUMP\t{}\t{}\t{}",
                label,
                topic_b64.join(","),
                data.to_xdr_base64(Limits::none()).unwrap()
            );
        }
    };

    std::println!("ADMIN\t{}", admin.to_string());
    std::println!("USER1\t{}", user1.to_string());
    std::println!("USER2\t{}", user2.to_string());
    std::println!("CONTRACT\t{}", contract_id.to_string());

    client.initialize(&admin);
    dump(&env, "init");
    client.whitelist_user(&admin, &user1);
    dump(&env, "wl_add");
    client.whitelist_user(&admin, &user2);
    dump(&env, "wl_add2");
    client.mint_asset(&admin, &user1, &1000);
    dump(&env, "mint");
    client.transfer(&user1, &user2, &250);
    dump(&env, "transfer");
    client.distribute_yield(&admin, &42);
    dump(&env, "yield");
    client.revoke_user(&admin, &user1);
    dump(&env, "wl_rev");
}
