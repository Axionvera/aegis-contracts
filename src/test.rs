#![cfg(test)]

use super::*;
use crate::asset::{AssetMintedEvent, AssetStatus, TransferEvent, YieldDistributedEvent};
use crate::capabilities::{
    CapabilityStatus, ComplianceCapabilities, ContractCapabilities, EventCapabilities,
    MetadataCapabilities, MintingCapabilities, PauseCapabilities, TransferCapabilities,
    CAPABILITY_SCHEMA_VERSION,
};
use crate::compliance::{UserWhitelistedEvent, WhitelistRevokedEvent};
use crate::eligibility::InvestorEligibility;
use crate::errors::Error;
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    vec, Address, Env, IntoVal, String, Symbol,
};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn setup() -> (Env, AegisContractClient<'static>, Address, Address, Address) {
    let env = Env::default();
    let contract_id = env.register_contract(None, AegisContract);
    let client = AegisContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    (env, client, admin, user1, user2)
}

// ─── Happy-path lifecycle ─────────────────────────────────────────────────────

#[test]
fn test_lifecycle() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    client.set_role(&admin, &user1, &Role::ComplianceOfficer);
    client.set_role(&admin, &user2, &Role::AssetManager);

    client.whitelist_user(&user1, &user1);
    client.whitelist_user(&user1, &user2);

    client.mint_asset(&user2, &user1, &1000);
    client.transfer(&user1, &user2, &250);

    assert_eq!(client.get_role_of(&user1), Role::ComplianceOfficer);
    assert_eq!(client.get_role_of(&user2), Role::AssetManager);
}

// ─── Wrong-caller: mint_asset ────────────────────────────────────────────────

#[test]
fn test_mint_reverts_without_asset_manager_role() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user2);

    // user1 has no role at all — mint should revert
    let result = client.try_mint_asset(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_mint_reverts_with_compliance_officer_role() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::ComplianceOfficer);
    client.whitelist_user(&admin, &user2);

    // ComplianceOfficer cannot mint — only AssetManager or Admin
    let result = client.try_mint_asset(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_mint_succeeds_with_asset_manager_role() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user2);

    let result = client.try_mint_asset(&user1, &user2, &100);
    assert!(result.is_ok());
}

#[test]
fn test_mint_reverts_with_invalid_amount() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user2);

    let result = client.try_mint_asset(&user1, &user2, &0);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn test_mint_reverts_when_receiver_not_whitelisted() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);

    // user2 was never whitelisted
    let result = client.try_mint_asset(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::ReceiverNotWhitelisted)));
}

#[test]
fn test_mint_succeeds_with_admin_role() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user2);

    // Admin can mint without an explicit AssetManager role assignment
    let result = client.try_mint_asset(&admin, &user2, &100);
    assert!(result.is_ok());
}

// ─── Transfer validation ──────────────────────────────────────────────────────

#[test]
fn test_transfer_reverts_with_invalid_amount() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);

    let result = client.try_transfer(&user1, &user2, &0);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn test_transfer_reverts_when_sender_not_whitelisted() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user2);

    // user1 was never whitelisted
    let result = client.try_transfer(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::SenderNotWhitelisted)));
}

#[test]
fn test_transfer_reverts_when_receiver_not_whitelisted() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user1);

    // user2 was never whitelisted
    let result = client.try_transfer(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::ReceiverNotWhitelisted)));
}

#[test]
fn test_transfer_reverts_with_insufficient_balance() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&user1, &user1, &50);

    // user1 only has a balance of 50
    let result = client.try_transfer(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::InsufficientBalance)));
}

// ─── Wrong-caller: distribute_yield ───────────────────────────────────────────

#[test]
fn test_distribute_yield_reverts_without_role() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    let result = client.try_distribute_yield(&user1, &100);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_distribute_yield_reverts_with_compliance_officer_role() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::ComplianceOfficer);

    let result = client.try_distribute_yield(&user1, &100);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_distribute_yield_reverts_with_invalid_amount() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);

    let result = client.try_distribute_yield(&user1, &0);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn test_distribute_yield_succeeds_with_asset_manager_role() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);

    let result = client.try_distribute_yield(&user1, &100);
    assert!(result.is_ok());
}

// ─── Wrong-caller: whitelist_user ─────────────────────────────────────────────

#[test]
fn test_whitelist_reverts_without_role() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // user2 has no role — whitelist should revert
    let result = client.try_whitelist_user(&user2, &user1);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_whitelist_reverts_with_asset_manager_role() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user2, &Role::AssetManager);

    // AssetManager cannot whitelist — only ComplianceOfficer or Admin
    let result = client.try_whitelist_user(&user2, &user1);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_whitelist_succeeds_with_compliance_officer_role() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user2, &Role::ComplianceOfficer);

    let result = client.try_whitelist_user(&user2, &user1);
    assert!(result.is_ok());
}

#[test]
fn test_whitelist_succeeds_with_admin_role() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    let result = client.try_whitelist_user(&admin, &user1);
    assert!(result.is_ok());
}

#[test]
fn test_whitelist_succeeds_with_emergency_officer_role() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user2, &Role::EmergencyOfficer);

    let result = client.try_whitelist_user(&user2, &user1);
    assert!(result.is_ok());
}

// ─── Wrong-caller: revoke_whitelist ───────────────────────────────────────────

#[test]
fn test_revoke_whitelist_reverts_without_role() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user1);

    // user2 has no role
    let result = client.try_revoke_whitelist(&user2, &user1);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_revoke_whitelist_succeeds_with_compliance_officer_role() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user2, &Role::ComplianceOfficer);
    client.whitelist_user(&user2, &user1);

    let result = client.try_revoke_whitelist(&user2, &user1);
    assert!(result.is_ok());
}

// ─── Role management ─────────────────────────────────────────────────────────

#[test]
fn test_set_role_reverts_for_non_admin() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // user1 is not admin — cannot assign roles
    let result = client.try_set_role(&user1, &user2, &Role::AssetManager);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_remove_role_reverts_for_non_admin() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user2, &Role::AssetManager);

    // user1 is not admin — cannot revoke roles
    let result = client.try_remove_role(&user1, &user2);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_remove_role_reverts_when_target_has_no_role() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // user2 has no role — revoking should revert
    let result = client.try_remove_role(&admin, &user2);
    assert_eq!(result, Err(Ok(Error::NoRoleToRevoke)));
}

#[test]
fn test_cannot_assign_admin_role_via_set_role() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // Trying to assign Admin role via set_role should revert
    let result = client.try_set_role(&admin, &user2, &Role::Admin);
    assert_eq!(result, Err(Ok(Error::CannotAssignAdminRole)));
}

#[test]
fn test_set_and_remove_role() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    client.set_role(&admin, &user2, &Role::AssetManager);
    assert_eq!(client.get_role_of(&user2), Role::AssetManager);

    client.remove_role(&admin, &user2);
    assert_eq!(client.get_role_of(&user2), Role::None);
}

#[test]
fn test_get_role_returns_none_for_unassigned() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    assert_eq!(client.get_role_of(&user2), Role::None);
}

// ─── Admin transfer (2-step) ─────────────────────────────────────────────────

#[test]
fn test_transfer_admin_reverts_for_non_admin() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    let result = client.try_transfer_admin(&user1, &user2);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_accept_admin_reverts_for_wrong_candidate() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.transfer_admin(&admin, &user1);

    // user2 tries to accept — should revert
    let result = client.try_accept_admin(&user2);
    assert_eq!(result, Err(Ok(Error::NotPendingCandidate)));
}

#[test]
fn test_accept_admin_reverts_without_pending_transfer() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // No transfer initiated — accept should revert
    let result = client.try_accept_admin(&user2);
    assert_eq!(result, Err(Ok(Error::NoPendingAdminTransfer)));
}

#[test]
fn test_full_admin_transfer() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // Step 1: Current admin initiates transfer
    client.transfer_admin(&admin, &user1);

    // Step 2: Candidate accepts
    client.accept_admin(&user1);

    // Verify: user1 is now admin, admin is no longer admin
    assert_eq!(client.get_role_of(&user1), Role::Admin);
    assert_eq!(client.get_role_of(&admin), Role::None);
}

// ─── Renounce admin ──────────────────────────────────────────────────────────

#[test]
fn test_renounce_admin_reverts_for_non_admin() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    let result = client.try_renounce_admin(&user1);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_renounce_admin_removes_admin() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    assert_eq!(client.get_role_of(&admin), Role::Admin);

    client.renounce_admin(&admin);
    assert_eq!(client.get_role_of(&admin), Role::None);
}

// ─── Double initialization ───────────────────────────────────────────────────

#[test]
fn test_double_initialization_reverts() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    let result = client.try_initialize(&user1);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

// ─── Storage: uninitialized contract ──────────────────────────────────────────

#[test]
fn test_set_role_reverts_when_not_initialized() {
    let (env, client, _admin, user1, user2) = setup();
    env.mock_all_auths();

    // No `initialize` call — the Admin key is missing from storage entirely.
    let result = client.try_set_role(&user1, &user2, &Role::AssetManager);
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

// ─── Pause: authorization ─────────────────────────────────────────────────────

#[test]
fn test_pause_reverts_for_unauthorized() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // user1 has no role — cannot pause
    let result = client.try_pause(&user1);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_pause_succeeds_for_admin() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    let result = client.try_pause(&admin);
    assert!(result.is_ok());
    assert!(client.is_paused());
}

#[test]
fn test_pause_succeeds_for_emergency_officer() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::EmergencyOfficer);

    let result = client.try_pause(&user1);
    assert!(result.is_ok());
    assert!(client.is_paused());
}

#[test]
fn test_pause_reverts_when_already_paused() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.pause(&admin);

    // Second pause should revert
    let result = client.try_pause(&admin);
    assert_eq!(result, Err(Ok(Error::AlreadyPaused)));
}

#[test]
fn test_unpause_reverts_for_non_admin() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.pause(&admin);

    // user1 is not admin — cannot unpause
    let result = client.try_unpause(&user1);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_unpause_reverts_for_emergency_officer() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::EmergencyOfficer);
    client.pause(&admin);

    // EmergencyOfficer can pause but cannot unpause
    let result = client.try_unpause(&user1);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_unpause_succeeds_for_admin() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.pause(&admin);
    assert!(client.is_paused());

    let result = client.try_unpause(&admin);
    assert!(result.is_ok());
    assert!(!client.is_paused());
}

#[test]
fn test_unpause_reverts_when_not_paused() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // Contract is not paused — unpause should revert
    let result = client.try_unpause(&admin);
    assert_eq!(result, Err(Ok(Error::NotPaused)));
}

// ─── Pause: blocked state-changing operations ─────────────────────────────────

#[test]
fn test_mint_blocked_when_paused() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user2);

    // Mint should succeed before pause
    let result = client.try_mint_asset(&user1, &user2, &100);
    assert!(result.is_ok());

    client.pause(&admin);

    // Mint should fail when paused
    let result = client.try_mint_asset(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::ContractPaused)));
}

#[test]
fn test_transfer_blocked_when_paused() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::ComplianceOfficer);
    client.set_role(&admin, &user2, &Role::AssetManager);
    client.whitelist_user(&user1, &user1);
    client.whitelist_user(&user1, &user2);
    client.mint_asset(&user2, &user1, &1000);

    // Transfer should succeed before pause
    let result = client.try_transfer(&user1, &user2, &250);
    assert!(result.is_ok());

    client.pause(&admin);

    // Transfer should fail when paused
    let result = client.try_transfer(&user1, &user2, &250);
    assert_eq!(result, Err(Ok(Error::ContractPaused)));
}

#[test]
fn test_whitelist_blocked_when_paused() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::ComplianceOfficer);

    client.pause(&admin);

    // Whitelist should fail when paused
    let result = client.try_whitelist_user(&user1, &user2);
    assert_eq!(result, Err(Ok(Error::ContractPaused)));
}

#[test]
fn test_revoke_whitelist_blocked_when_paused() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::ComplianceOfficer);
    client.whitelist_user(&user1, &user2);

    client.pause(&admin);

    // Revoke whitelist should fail when paused
    let result = client.try_revoke_whitelist(&user1, &user2);
    assert_eq!(result, Err(Ok(Error::ContractPaused)));
}

#[test]
fn test_distribute_yield_blocked_when_paused() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);

    client.pause(&admin);

    // Distribute yield should fail when paused
    let result = client.try_distribute_yield(&user1, &100);
    assert_eq!(result, Err(Ok(Error::ContractPaused)));
}

#[test]
fn test_set_role_blocked_when_paused() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    client.pause(&admin);

    // set_role is an admin operation — also blocked during pause
    let result = client.try_set_role(&admin, &user1, &Role::AssetManager);
    assert_eq!(result, Err(Ok(Error::ContractPaused)));
}

#[test]
fn test_remove_role_blocked_when_paused() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user2, &Role::AssetManager);

    client.pause(&admin);

    // remove_role is also blocked during pause
    let result = client.try_remove_role(&admin, &user2);
    assert_eq!(result, Err(Ok(Error::ContractPaused)));
}

// ─── Pause: read functions remain available ───────────────────────────────────

#[test]
fn test_read_functions_available_when_paused() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::ComplianceOfficer);
    client.set_role(&admin, &user2, &Role::AssetManager);
    client.whitelist_user(&user1, &user1);
    client.whitelist_user(&user1, &user2);
    client.mint_asset(&user2, &user1, &1000);

    client.pause(&admin);

    // Read functions should all still work
    assert!(client.is_paused());
    assert_eq!(client.get_role_of(&user1), Role::ComplianceOfficer);
    assert_eq!(client.get_role_of(&user2), Role::AssetManager);
    assert_eq!(client.get_balance_of(&user1), 1000);
    assert_eq!(client.get_balance_of(&user2), 0);
    assert!(client.is_whitelisted(&user1));
    assert!(client.is_whitelisted(&user2));
    assert_eq!(client.get_total_supply(), 1000);
}

// ─── Pause: full lifecycle ────────────────────────────────────────────────────

#[test]
fn test_pause_unpause_full_lifecycle() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::ComplianceOfficer);
    client.set_role(&admin, &user2, &Role::AssetManager);
    client.whitelist_user(&user1, &user1);
    client.whitelist_user(&user1, &user2);

    // Operations work before pause
    client.mint_asset(&user2, &user1, &1000);
    client.transfer(&user1, &user2, &250);
    assert_eq!(client.get_balance_of(&user1), 750);
    assert_eq!(client.get_balance_of(&user2), 250);

    // Pause blocks operations
    client.pause(&admin);
    assert!(client.is_paused());

    let result = client.try_mint_asset(&user2, &user1, &500);
    assert_eq!(result, Err(Ok(Error::ContractPaused)));
    let result = client.try_transfer(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::ContractPaused)));
    let result = client.try_whitelist_user(&user1, &Address::generate(&env));
    assert_eq!(result, Err(Ok(Error::ContractPaused)));

    // Unpause restores operations
    client.unpause(&admin);
    assert!(!client.is_paused());

    client.mint_asset(&user2, &user1, &500);
    client.transfer(&user1, &user2, &100);
    assert_eq!(client.get_balance_of(&user1), 1150);
    assert_eq!(client.get_balance_of(&user2), 350);
}

// ─── Event compatibility: compliance & minting ────────────────────────────────
//
// These tests lock down the exact topic and payload shape of each event so
// that downstream SDKs, dashboards, and indexers relying on this contract's
// event schema get a compile-time-checked regression signal if the shape
// ever drifts. `env.events().all()` returns only the events published by the
// most recent top-level invocation, so each test calls the action under
// test as the last client call before asserting.

#[test]
fn test_whitelist_user_emits_event() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user1);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("user_whitelisted",).into_val(&env),
                UserWhitelistedEvent {
                    caller: admin,
                    user: user1,
                }
                .into_val(&env),
            ),
        ]
    );
}

#[test]
fn test_revoke_whitelist_emits_event() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user1);
    client.revoke_whitelist(&admin, &user1);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("whitelist_revoked",).into_val(&env),
                WhitelistRevokedEvent {
                    caller: admin,
                    user: user1,
                }
                .into_val(&env),
            ),
        ]
    );
}

#[test]
fn test_mint_asset_emits_event_with_running_supply() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user2);

    // First mint establishes a baseline supply.
    client.mint_asset(&user1, &user2, &400);

    // Second mint's event should reflect the *cumulative* total supply, not
    // just the minted amount.
    client.mint_asset(&user1, &user2, &600);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("asset_minted",).into_val(&env),
                AssetMintedEvent {
                    caller: user1,
                    to: user2,
                    amount: 600,
                    total_supply: 1000,
                }
                .into_val(&env),
            ),
        ]
    );
}

#[test]
fn test_transfer_emits_event() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&user1, &user1, &1000);

    client.transfer(&user1, &user2, &250);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("transfer",).into_val(&env),
                TransferEvent {
                    from: user1,
                    to: user2,
                    amount: 250,
                }
                .into_val(&env),
            ),
        ]
    );
}

#[test]
fn test_blocked_transfer_emits_no_event() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user1);
    // user2 is deliberately left off the whitelist.

    // Reverted invocations discard their events entirely — the standardized
    // `ReceiverNotWhitelisted` error code is the only observable signal for
    // a compliance-restricted transfer (see docs/events.md).
    let result = client.try_transfer(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::ReceiverNotWhitelisted)));
    assert_eq!(env.events().all().events().len(), 0);
}

#[test]
fn test_distribute_yield_emits_event() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);

    client.distribute_yield(&user1, &500);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("yield_distributed",).into_val(&env),
                YieldDistributedEvent {
                    caller: user1,
                    amount: 500,
                }
                .into_val(&env),
            ),
        ]
    );
}

// ─── Supply cap amendment governance (#32) ────────────────────────────────────

#[test]
fn test_supply_cap_default_is_unbounded() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user2);

    // No cap set yet → minting is not blocked by a cap.
    assert_eq!(client.get_supply_cap(), 0);
    assert!(client.get_pending_supply_cap().is_none());
    let r = client.try_mint_asset(&admin, &user2, &1000);
    assert!(r.is_ok());
}

#[test]
fn test_supply_cap_requires_two_step_governance() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user2);

    // Non-admin cannot propose.
    let r = client.try_propose_supply_cap(&user1, &1000);
    assert!(r.is_err());

    // Propose, then cap is still not active (no-op until accepted).
    client.propose_supply_cap(&admin, &1000);
    assert_eq!(client.get_supply_cap(), 0);
    assert_eq!(client.get_pending_supply_cap(), Some(1000));

    // Mint below proposed cap still works (active cap is still 0).
    let r = client.try_mint_asset(&admin, &user2, &500);
    assert!(r.is_ok());

    // Accept activates the cap.
    client.accept_supply_cap(&admin);
    assert_eq!(client.get_supply_cap(), 1000);
    assert!(client.get_pending_supply_cap().is_none());

    // Now minting above the cap is rejected.
    let r = client.try_mint_asset(&admin, &user2, &600);
    assert!(r.is_err());

    // Minting up to the cap is allowed.
    let r = client.try_mint_asset(&admin, &user2, &500);
    assert!(r.is_ok());
}

#[test]
fn test_supply_cap_proposal_cancel() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    client.propose_supply_cap(&admin, &500);
    assert_eq!(client.get_pending_supply_cap(), Some(500));

    // Cancel clears the proposal; accept should then fail.
    client.cancel_supply_cap_proposal(&admin);
    assert!(client.get_pending_supply_cap().is_none());

    let r = client.try_accept_supply_cap(&admin);
    assert!(r.is_err());
    assert_eq!(client.get_supply_cap(), 0);
}

#[test]
fn test_supply_cap_noop_rejected() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    // Proposing the same value as the active cap (0) is a no-op → rejected.
    let r = client.try_propose_supply_cap(&admin, &0);
    assert!(r.is_err());
}

#[test]
fn test_supply_cap_negative_rejected() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    let r = client.try_propose_supply_cap(&admin, &-1);
    assert!(r.is_err());
}

#[test]
fn test_supply_cap_lowering_below_supply_blocks_future_mints() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user2);

    client.mint_asset(&admin, &user2, &1000);
    assert_eq!(client.get_total_supply(), 1000);

    // Lower the cap below current supply (allowed — does not burn supply).
    client.propose_supply_cap(&admin, &500);
    client.accept_supply_cap(&admin);
    assert_eq!(client.get_supply_cap(), 500);

    // Existing supply (1000) now exceeds the cap; further mints are blocked
    // until supply falls or the cap is raised.
    let r = client.try_mint_asset(&admin, &user2, &1);
    assert!(r.is_err());
}

#[test]
fn test_supply_cap_zero_mint_rejected_without_state_change() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user2);
    client.propose_supply_cap(&admin, &1000);
    client.accept_supply_cap(&admin);

    assert_eq!(client.get_total_supply(), 0);
    assert_eq!(client.get_balance_of(&user2), 0);

    let r = client.try_mint_asset(&admin, &user2, &0);
    assert_eq!(r, Err(Ok(Error::InvalidAmount)));

    // Failed mint must not mutate supply or recipient balance.
    assert_eq!(client.get_total_supply(), 0);
    assert_eq!(client.get_balance_of(&user2), 0);
}

#[test]
fn test_supply_cap_boundary_and_exceeded_preserve_state() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user2);
    client.propose_supply_cap(&admin, &1000);
    client.accept_supply_cap(&admin);

    // Exactly at cap must succeed.
    let r = client.try_mint_asset(&admin, &user2, &1000);
    assert!(r.is_ok());
    assert_eq!(client.get_total_supply(), 1000);
    assert_eq!(client.get_balance_of(&user2), 1000);

    // Any additional mint must fail and keep state unchanged.
    let r = client.try_mint_asset(&admin, &user2, &1);
    assert!(r.is_err());
    assert_eq!(client.get_total_supply(), 1000);
    assert_eq!(client.get_balance_of(&user2), 1000);
}

#[test]
fn test_supply_cap_repeated_minting_near_cap_then_rejects() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user2);
    client.propose_supply_cap(&admin, &1000);
    client.accept_supply_cap(&admin);

    // Repeated mints approaching the cap.
    let matrix = [300_i128, 300_i128, 399_i128, 1_i128];
    for amount in matrix {
        let r = client.try_mint_asset(&admin, &user2, &amount);
        assert!(r.is_ok());
    }
    assert_eq!(client.get_total_supply(), 1000);
    assert_eq!(client.get_balance_of(&user2), 1000);

    // Any extra amount must fail with no state drift.
    let r = client.try_mint_asset(&admin, &user2, &1);
    assert!(r.is_err());
    assert_eq!(client.get_total_supply(), 1000);
    assert_eq!(client.get_balance_of(&user2), 1000);
}

#[test]
fn test_supply_cap_overflow_like_mint_keeps_state_consistent() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user2);
    client.propose_supply_cap(&admin, &i128::MAX);
    client.accept_supply_cap(&admin);

    // Reach the practical numeric boundary first.
    let r = client.try_mint_asset(&admin, &user2, &i128::MAX);
    assert!(r.is_ok());
    assert_eq!(client.get_total_supply(), i128::MAX);
    assert_eq!(client.get_balance_of(&user2), i128::MAX);

    // Next mint would require i128::MAX + 1 internally (overflow-like path).
    // The call must fail and preserve the pre-call state.
    let r = client.try_mint_asset(&admin, &user2, &1);
    assert!(r.is_err());
    assert_eq!(client.get_total_supply(), i128::MAX);
    assert_eq!(client.get_balance_of(&user2), i128::MAX);
}

// ─── Asset lifecycle invariants (#55) ─────────────────────────────────────────

#[test]
fn test_asset_lifecycle_defaults_to_active() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    assert_eq!(client.get_asset_status(), AssetStatus::Active);
}

#[test]
fn test_asset_lifecycle_wrong_caller_transition_rejected() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // user1 has no emergency/admin privileges for lifecycle transitions.
    let r = client.try_set_asset_status(&user1, &AssetStatus::Paused);
    assert_eq!(r, Err(Ok(Error::Unauthorized)));
    assert_eq!(client.get_asset_status(), AssetStatus::Active);
}

#[test]
fn test_asset_lifecycle_invalid_transition_rejected_with_state_consistency() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_asset_status(&admin, &AssetStatus::Retired);
    assert_eq!(client.get_asset_status(), AssetStatus::Retired);

    // Retired is terminal in the lifecycle model.
    let r = client.try_set_asset_status(&admin, &AssetStatus::Active);
    assert_eq!(r, Err(Ok(Error::InvalidAssetStatusTransition)));
    assert_eq!(client.get_asset_status(), AssetStatus::Retired);
}

#[test]
fn test_asset_paused_blocks_mint_and_transfer_with_unchanged_state() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&user1, &user1, &500);

    client.set_asset_status(&admin, &AssetStatus::Paused);

    let supply_before = client.get_total_supply();
    let user1_before = client.get_balance_of(&user1);
    let user2_before = client.get_balance_of(&user2);

    let mint_r = client.try_mint_asset(&user1, &user2, &10);
    assert_eq!(mint_r, Err(Ok(Error::AssetNotActive)));

    let transfer_r = client.try_transfer(&user1, &user2, &10);
    assert_eq!(transfer_r, Err(Ok(Error::AssetNotActive)));

    // Failed operations must not mutate balances/supply.
    assert_eq!(client.get_total_supply(), supply_before);
    assert_eq!(client.get_balance_of(&user1), user1_before);
    assert_eq!(client.get_balance_of(&user2), user2_before);
}

#[test]
fn test_asset_retired_blocks_mint_transfer_and_metadata_update() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&user1, &user1, &200);

    client.set_asset_status(&admin, &AssetStatus::Retired);
    let supply_before = client.get_total_supply();
    let user1_before = client.get_balance_of(&user1);
    let user2_before = client.get_balance_of(&user2);
    let metadata_before = client.get_asset_metadata();

    let mint_r = client.try_mint_asset(&user1, &user2, &1);
    assert_eq!(mint_r, Err(Ok(Error::AssetNotActive)));

    let transfer_r = client.try_transfer(&user1, &user2, &1);
    assert_eq!(transfer_r, Err(Ok(Error::AssetNotActive)));

    let metadata_r = client.try_update_asset_metadata(
        &user1,
        &String::from_str(&env, "Retired Name"),
        &String::from_str(&env, "RET"),
        &String::from_str(&env, "ipfs://retired"),
    );
    assert_eq!(metadata_r, Err(Ok(Error::AssetMetadataUpdateBlocked)));

    // Failed operations keep all tracked state unchanged.
    assert_eq!(client.get_total_supply(), supply_before);
    assert_eq!(client.get_balance_of(&user1), user1_before);
    assert_eq!(client.get_balance_of(&user2), user2_before);
    assert_eq!(client.get_asset_metadata(), metadata_before);
}

#[test]
fn test_asset_blocked_blocks_mint_and_transfer() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&user1, &user1, &100);

    client.set_asset_status(&admin, &AssetStatus::Blocked);
    let supply_before = client.get_total_supply();
    let user1_before = client.get_balance_of(&user1);
    let user2_before = client.get_balance_of(&user2);
    let metadata_before = client.get_asset_metadata();

    let mint_r = client.try_mint_asset(&user1, &user2, &10);
    assert_eq!(mint_r, Err(Ok(Error::AssetNotActive)));

    let transfer_r = client.try_transfer(&user1, &user2, &10);
    assert_eq!(transfer_r, Err(Ok(Error::AssetNotActive)));

    let metadata_r = client.try_update_asset_metadata(
        &user1,
        &String::from_str(&env, "Blocked Name"),
        &String::from_str(&env, "BLK"),
        &String::from_str(&env, "ipfs://blocked"),
    );
    assert_eq!(metadata_r, Err(Ok(Error::AssetMetadataUpdateBlocked)));

    assert_eq!(client.get_total_supply(), supply_before);
    assert_eq!(client.get_balance_of(&user1), user1_before);
    assert_eq!(client.get_balance_of(&user2), user2_before);
    assert_eq!(client.get_asset_metadata(), metadata_before);
}

#[test]
fn test_asset_metadata_update_allowed_in_active_and_paused() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);

    let r = client.try_update_asset_metadata(
        &user1,
        &String::from_str(&env, "Aegis Real Estate Trust"),
        &String::from_str(&env, "AERT"),
        &String::from_str(&env, "ipfs://aegis/asset/1"),
    );
    assert!(r.is_ok());

    client.set_asset_status(&admin, &AssetStatus::Paused);
    let r = client.try_update_asset_metadata(
        &user1,
        &String::from_str(&env, "Aegis Real Estate Trust v2"),
        &String::from_str(&env, "AERT"),
        &String::from_str(&env, "ipfs://aegis/asset/2"),
    );
    assert!(r.is_ok());
}

#[test]
fn test_asset_admin_transfer_still_works_when_asset_paused() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_asset_status(&admin, &AssetStatus::Paused);

    // Asset lifecycle pause restricts mint/transfer, but governance can still
    // rotate admin keys for operational recovery.
    let r = client.try_transfer_admin(&admin, &user1);
    assert!(r.is_ok());
    let r = client.try_accept_admin(&user1);
    assert!(r.is_ok());
    assert_eq!(client.get_role_of(&user1), Role::Admin);
}

#[test]
fn test_asset_admin_transfer_still_works_when_asset_blocked_or_retired() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_asset_status(&admin, &AssetStatus::Blocked);

    // Governance/admin changes are still allowed in blocked status.
    let r = client.try_transfer_admin(&admin, &user1);
    assert!(r.is_ok());
    let r = client.try_accept_admin(&user1);
    assert!(r.is_ok());
    assert_eq!(client.get_role_of(&user1), Role::Admin);

    client.set_asset_status(&user1, &AssetStatus::Retired);
    assert_eq!(client.get_asset_status(), AssetStatus::Retired);

    // Governance/admin changes are still allowed in retired status.
    let r = client.try_transfer_admin(&user1, &user2);
    assert!(r.is_ok());
    let r = client.try_accept_admin(&user2);
    assert!(r.is_ok());
    assert_eq!(client.get_role_of(&user2), Role::Admin);
}

// ─── Investor holding restriction checks (#33) ───────────────────────────────

#[test]
fn test_holding_cap_default_is_unrestricted() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user2);

    // No holding cap set → minting to a holder is not blocked by a cap.
    assert_eq!(client.get_holding_cap(), 0);
    let r = client.try_mint_asset(&user1, &user2, &10000);
    assert!(r.is_ok());
    assert_eq!(client.get_balance_of(&user2), 10000);
}

#[test]
fn test_holding_cap_blocks_mint_over_limit() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user2);

    // Propose + accept a holding cap of 500.
    client.propose_holding_cap(&admin, &500);
    client.accept_holding_cap(&admin);
    assert_eq!(client.get_holding_cap(), 500);

    // Mint up to the cap is allowed.
    let r = client.try_mint_asset(&user1, &user2, &500);
    assert!(r.is_ok());
    assert_eq!(client.get_balance_of(&user2), 500);

    // Mint that would push the holder over the cap is rejected.
    let r = client.try_mint_asset(&user1, &user2, &1);
    assert!(r.is_err());
}

#[test]
fn test_holding_cap_blocks_transfer_over_limit() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);

    // Give user1 a balance, then cap user2's holding at 300.
    client.mint_asset(&user1, &user1, &1000);
    client.propose_holding_cap(&admin, &300);
    client.accept_holding_cap(&admin);

    // Transfer that would push user2 over 300 is rejected.
    let r = client.try_transfer(&user1, &user2, &301);
    assert!(r.is_err());

    // Transfer within the cap is allowed.
    let r = client.try_transfer(&user1, &user2, &300);
    assert!(r.is_ok());
    assert_eq!(client.get_balance_of(&user2), 300);
}

#[test]
fn test_holding_cap_governance_requires_admin_and_two_steps() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // Non-admin cannot propose.
    let r = client.try_propose_holding_cap(&user1, &100);
    assert!(r.is_err());

    // Propose, then cap is still not active.
    client.propose_holding_cap(&admin, &100);
    assert_eq!(client.get_holding_cap(), 0);
    assert_eq!(client.get_pending_holding_cap(), Some(100));

    // Accept activates it.
    client.accept_holding_cap(&admin);
    assert_eq!(client.get_holding_cap(), 100);
    assert!(client.get_pending_holding_cap().is_none());

    // No-op proposal (== active) is rejected.
    let r = client.try_propose_holding_cap(&admin, &100);
    assert!(r.is_err());

    // Negative proposal is rejected.
    let r = client.try_propose_holding_cap(&admin, &-1);
    assert!(r.is_err());
}

// ─── Investor eligibility read helpers (#14) ─────────────────────────────────

#[test]
fn test_eligibility_default_state_is_ineligible() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // user2 was never whitelisted and has never held a balance.
    let elig = client.get_investor_eligibility(&user2);
    assert_eq!(
        elig,
        InvestorEligibility {
            whitelisted: false,
            contract_paused: false,
            balance: 0,
            holding_cap: 0,
            remaining_capacity: None,
            can_receive: false,
            can_send: false,
        }
    );
}

#[test]
fn test_eligibility_reflects_whitelisted_holder_with_balance() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&user1, &user2, &500);

    let elig = client.get_investor_eligibility(&user2);
    assert_eq!(
        elig,
        InvestorEligibility {
            whitelisted: true,
            contract_paused: false,
            balance: 500,
            holding_cap: 0,
            remaining_capacity: None,
            can_receive: true,
            can_send: true,
        }
    );
}

#[test]
fn test_eligibility_reflects_holding_cap_headroom() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user2);
    client.propose_holding_cap(&admin, &500);
    client.accept_holding_cap(&admin);

    // Partially filled: headroom remains, so the investor can still receive.
    client.mint_asset(&user1, &user2, &300);
    let elig = client.get_investor_eligibility(&user2);
    assert_eq!(elig.holding_cap, 500);
    assert_eq!(elig.remaining_capacity, Some(200));
    assert!(elig.can_receive);
    assert!(elig.can_send);

    // Filled to the cap: no headroom left, so the investor cannot receive
    // further tokens, but can still send out of their existing balance.
    client.mint_asset(&user1, &user2, &200);
    let elig = client.get_investor_eligibility(&user2);
    assert_eq!(elig.balance, 500);
    assert_eq!(elig.remaining_capacity, Some(0));
    assert!(!elig.can_receive);
    assert!(elig.can_send);
}

#[test]
fn test_eligibility_reflects_paused_contract() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&user1, &user2, &500);

    client.pause(&admin);

    // The read helper itself must remain callable while paused...
    let elig = client.get_investor_eligibility(&user2);
    // ...but reflects that no transfer/mint can currently succeed.
    assert!(elig.whitelisted);
    assert!(elig.contract_paused);
    assert_eq!(elig.balance, 500);
    assert!(!elig.can_receive);
    assert!(!elig.can_send);
}

#[test]
fn test_check_transfer_eligibility_true_for_eligible_transfer() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&user1, &user1, &1000);

    assert!(client.check_transfer_eligibility(&user1, &user2, &250));

    // The check does not mutate state: the actual transfer still succeeds
    // afterwards for the same amount.
    let result = client.try_transfer(&user1, &user2, &250);
    assert!(result.is_ok());
}

#[test]
fn test_check_transfer_eligibility_false_when_invalid_amount() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);

    assert!(!client.check_transfer_eligibility(&user1, &user2, &0));
    assert!(!client.check_transfer_eligibility(&user1, &user2, &-10));
}

#[test]
fn test_check_transfer_eligibility_false_when_sender_not_whitelisted() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user2);
    // user1 was never whitelisted.

    assert!(!client.check_transfer_eligibility(&user1, &user2, &100));
}

#[test]
fn test_check_transfer_eligibility_false_when_receiver_not_whitelisted() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user1);
    // user2 was never whitelisted.

    assert!(!client.check_transfer_eligibility(&user1, &user2, &100));
}

#[test]
fn test_check_transfer_eligibility_false_when_insufficient_balance() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&user1, &user1, &50);

    // user1 only has a balance of 50.
    assert!(!client.check_transfer_eligibility(&user1, &user2, &100));
}

#[test]
fn test_check_transfer_eligibility_false_when_over_holding_cap() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&user1, &user1, &1000);

    client.propose_holding_cap(&admin, &300);
    client.accept_holding_cap(&admin);

    // Would push user2 over the 300 cap.
    assert!(!client.check_transfer_eligibility(&user1, &user2, &301));
    // Exactly at the cap is still eligible.
    assert!(client.check_transfer_eligibility(&user1, &user2, &300));
}

#[test]
fn test_check_transfer_eligibility_false_when_contract_paused() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&user1, &user1, &1000);

    client.pause(&admin);

    // The read helper itself must remain callable while paused, but must
    // reflect that transfers cannot currently succeed.
    assert!(!client.check_transfer_eligibility(&user1, &user2, &100));
}

#[test]
fn test_check_transfer_eligibility_matches_actual_transfer_outcomes() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user1);
    client.mint_asset(&user1, &user1, &1000);
    // user2 deliberately left off the whitelist.

    assert!(!client.check_transfer_eligibility(&user1, &user2, &100));
    let result = client.try_transfer(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::ReceiverNotWhitelisted)));
}

// ─── Contract capability flags (#82) ─────────────────────────────────────────
//
// These tests lock down the read-only capability surface that SDK and
// dashboard clients feature-gate against. They assert three things: the
// default capability state on a fresh deployment, that the helper never
// mutates state, and that unsupported/planned states are distinguishable.

/// The full expected capability descriptor for a freshly deployed contract
/// with no admin, no caps, no metadata, and no pause. Built as a helper so
/// each test can assert against an exhaustive struct literal — adding a
/// field to any capability struct fails compilation here until the expected
/// default is declared, which is the point.
fn default_capabilities(env: &Env) -> ContractCapabilities {
    ContractCapabilities {
        capability_version: CAPABILITY_SCHEMA_VERSION,
        contract_version: String::from_str(env, env!("CARGO_PKG_VERSION")),
        initialized: false,
        rbac: CapabilityStatus::Supported,
        two_step_governance: CapabilityStatus::Supported,
        sep41_token_interface: CapabilityStatus::Planned,
        compliance: ComplianceCapabilities {
            module_enabled: true,
            whitelist: CapabilityStatus::Supported,
            whitelist_revocation: CapabilityStatus::Supported,
            batch_whitelisting: CapabilityStatus::Planned,
            investor_tiers: CapabilityStatus::Unsupported,
            eligibility_reads: CapabilityStatus::Supported,
            enforced_on_mint: true,
            enforced_on_transfer: true,
        },
        minting: MintingCapabilities {
            module_enabled: true,
            minting: CapabilityStatus::Supported,
            burning: CapabilityStatus::Unsupported,
            supply_cap: CapabilityStatus::Supported,
            supply_cap_enforced: false,
            yield_distribution: CapabilityStatus::Planned,
        },
        transfers: TransferCapabilities {
            module_enabled: true,
            transfers: CapabilityStatus::Supported,
            holding_cap: CapabilityStatus::Supported,
            holding_cap_enforced: false,
            allowances: CapabilityStatus::Planned,
            transfer_from: CapabilityStatus::Planned,
            transfer_fees: CapabilityStatus::Planned,
            transfer_eligibility_check: CapabilityStatus::Supported,
        },
        pause: PauseCapabilities {
            module_enabled: true,
            global_pause: CapabilityStatus::Supported,
            paused: false,
            asset_lifecycle: CapabilityStatus::Supported,
            asset_active: true,
            operations_enabled: true,
        },
        metadata: MetadataCapabilities {
            module_enabled: true,
            name_and_symbol: CapabilityStatus::Supported,
            metadata_uri: CapabilityStatus::Supported,
            decimals: CapabilityStatus::Planned,
            metadata_configured: false,
            lifecycle_restricted: true,
        },
        events: EventCapabilities {
            module_enabled: true,
            compliance_events: CapabilityStatus::Supported,
            minting_events: CapabilityStatus::Supported,
            transfer_events: CapabilityStatus::Supported,
            admin_events: CapabilityStatus::Supported,
            governance_events: CapabilityStatus::Supported,
            asset_lifecycle_events: CapabilityStatus::Supported,
            transfer_restriction_events: CapabilityStatus::Unsupported,
            asset_registered_event: CapabilityStatus::Planned,
        },
    }
}

#[test]
fn test_capabilities_default_state_before_initialize() {
    let (env, client, _admin, _user1, _user2) = setup();

    // Callable on a bare, uninitialized deployment — no admin in storage,
    // no auth mocked, and it must not revert with NotInitialized.
    let caps = client.get_capabilities();
    assert_eq!(caps, default_capabilities(&env));
}

#[test]
fn test_capabilities_default_state_after_initialize() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // Only `initialized` flips; every static capability is unchanged.
    let mut expected = default_capabilities(&env);
    expected.initialized = true;
    assert_eq!(client.get_capabilities(), expected);
}

#[test]
fn test_capabilities_represent_all_required_domains() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    let caps = client.get_capabilities();

    // Compliance, minting, transfer, pause, metadata, and event support are
    // each represented by an enabled module with a supported core behaviour.
    assert!(caps.compliance.module_enabled);
    assert_eq!(caps.compliance.whitelist, CapabilityStatus::Supported);
    assert!(caps.minting.module_enabled);
    assert_eq!(caps.minting.minting, CapabilityStatus::Supported);
    assert!(caps.transfers.module_enabled);
    assert_eq!(caps.transfers.transfers, CapabilityStatus::Supported);
    assert!(caps.pause.module_enabled);
    assert_eq!(caps.pause.global_pause, CapabilityStatus::Supported);
    assert!(caps.metadata.module_enabled);
    assert_eq!(caps.metadata.name_and_symbol, CapabilityStatus::Supported);
    assert!(caps.events.module_enabled);
    assert_eq!(caps.events.compliance_events, CapabilityStatus::Supported);

    // Versioning is represented and non-empty.
    assert_eq!(caps.capability_version, CAPABILITY_SCHEMA_VERSION);
    assert_eq!(
        caps.contract_version,
        String::from_str(&env, env!("CARGO_PKG_VERSION"))
    );
    assert!(!caps.contract_version.is_empty());
}

#[test]
fn test_capabilities_read_does_not_mutate_state() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&user1, &user2, &750);
    client.propose_supply_cap(&admin, &5000);

    let supply_before = client.get_total_supply();
    let balance_before = client.get_balance_of(&user2);
    let status_before = client.get_asset_status();
    let metadata_before = client.get_asset_metadata();
    let pending_cap_before = client.get_pending_supply_cap();
    let ledger_before = env.to_ledger_snapshot();

    // Call every capability read repeatedly.
    for _ in 0..3 {
        let _ = client.get_capabilities();
        let _ = client.supports_capability(&Symbol::new(&env, "minting"));
        let _ = client.get_capability_keys();
    }

    // No storage entry anywhere in the ledger may have changed.
    assert_eq!(
        env.to_ledger_snapshot().ledger_entries,
        ledger_before.ledger_entries
    );
    assert_eq!(client.get_total_supply(), supply_before);
    assert_eq!(client.get_balance_of(&user2), balance_before);
    assert_eq!(client.get_asset_status(), status_before);
    assert_eq!(client.get_asset_metadata(), metadata_before);
    assert_eq!(client.get_pending_supply_cap(), pending_cap_before);
    assert_eq!(client.get_role_of(&user1), Role::AssetManager);
    assert!(client.is_whitelisted(&user2));
    assert!(!client.is_paused());

    // A pure read publishes no events.
    let _ = client.get_capabilities();
    assert_eq!(env.events().all().events().len(), 0);
}

#[test]
fn test_capabilities_remain_readable_when_paused() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.pause(&admin);

    // The read helper itself must stay callable while paused...
    let caps = client.get_capabilities();

    // ...and reflect that operations are globally halted, while the static
    // pause *capability* remains Supported (it exists; it is simply active).
    assert!(caps.pause.paused);
    assert!(caps.pause.asset_active);
    assert!(!caps.pause.operations_enabled);
    assert_eq!(caps.pause.global_pause, CapabilityStatus::Supported);
    assert_eq!(caps.compliance.whitelist, CapabilityStatus::Supported);
}

#[test]
fn test_capabilities_reflect_asset_lifecycle_state() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_asset_status(&admin, &AssetStatus::Retired);

    let caps = client.get_capabilities();
    assert!(!caps.pause.asset_active);
    assert!(!caps.pause.operations_enabled);
    // The lifecycle *capability* still exists even in a terminal status.
    assert_eq!(caps.pause.asset_lifecycle, CapabilityStatus::Supported);
}

#[test]
fn test_capabilities_reflect_active_supply_and_holding_caps() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // Defaults: capability supported, but not currently enforced.
    let caps = client.get_capabilities();
    assert_eq!(caps.minting.supply_cap, CapabilityStatus::Supported);
    assert!(!caps.minting.supply_cap_enforced);
    assert_eq!(caps.transfers.holding_cap, CapabilityStatus::Supported);
    assert!(!caps.transfers.holding_cap_enforced);

    // A pending proposal is not yet active — the runtime flag must not flip
    // until the 2-step governance flow completes.
    client.propose_supply_cap(&admin, &1000);
    assert!(!client.get_capabilities().minting.supply_cap_enforced);

    client.accept_supply_cap(&admin);
    client.propose_holding_cap(&admin, &250);
    client.accept_holding_cap(&admin);

    let caps = client.get_capabilities();
    assert!(caps.minting.supply_cap_enforced);
    assert!(caps.transfers.holding_cap_enforced);
}

#[test]
fn test_capabilities_reflect_metadata_configuration() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);

    // Unset metadata → capability supported, but not yet configured.
    let caps = client.get_capabilities();
    assert_eq!(caps.metadata.name_and_symbol, CapabilityStatus::Supported);
    assert!(!caps.metadata.metadata_configured);

    client.update_asset_metadata(
        &user1,
        &String::from_str(&env, "Aegis Real Estate Trust"),
        &String::from_str(&env, "AERT"),
        &String::from_str(&env, "ipfs://aegis/asset/1"),
    );

    assert!(client.get_capabilities().metadata.metadata_configured);
}

#[test]
fn test_capabilities_metadata_not_configured_when_symbol_blank() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);

    // A name without a symbol is not a usable metadata set for a dashboard.
    client.update_asset_metadata(
        &user1,
        &String::from_str(&env, "Aegis Real Estate Trust"),
        &String::from_str(&env, ""),
        &String::from_str(&env, ""),
    );

    assert!(!client.get_capabilities().metadata.metadata_configured);
}

#[test]
fn test_capabilities_unsupported_states_are_explicit() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    let caps = client.get_capabilities();

    // Structurally impossible / deliberately out of scope → Unsupported.
    assert_eq!(caps.minting.burning, CapabilityStatus::Unsupported);
    assert_eq!(
        caps.compliance.investor_tiers,
        CapabilityStatus::Unsupported
    );
    assert_eq!(
        caps.events.transfer_restriction_events,
        CapabilityStatus::Unsupported
    );

    // Known, tracked gaps → Planned (distinct from Unsupported, so a client
    // can render "coming soon" instead of hiding the control outright).
    assert_eq!(caps.transfers.allowances, CapabilityStatus::Planned);
    assert_eq!(caps.transfers.transfer_from, CapabilityStatus::Planned);
    assert_eq!(caps.transfers.transfer_fees, CapabilityStatus::Planned);
    assert_eq!(caps.metadata.decimals, CapabilityStatus::Planned);
    assert_eq!(caps.sep41_token_interface, CapabilityStatus::Planned);
    assert_eq!(
        caps.compliance.batch_whitelisting,
        CapabilityStatus::Planned
    );
    assert_eq!(caps.minting.yield_distribution, CapabilityStatus::Planned);
    assert_eq!(
        caps.events.asset_registered_event,
        CapabilityStatus::Planned
    );

    // The three states must be mutually distinguishable.
    assert_ne!(CapabilityStatus::Planned, CapabilityStatus::Unsupported);
    assert_ne!(CapabilityStatus::Supported, CapabilityStatus::Planned);
}

#[test]
fn test_supports_capability_resolves_known_keys() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    assert_eq!(
        client.supports_capability(&Symbol::new(&env, "whitelist")),
        CapabilityStatus::Supported
    );
    assert_eq!(
        client.supports_capability(&Symbol::new(&env, "minting")),
        CapabilityStatus::Supported
    );
    assert_eq!(
        client.supports_capability(&Symbol::new(&env, "transfers")),
        CapabilityStatus::Supported
    );
    assert_eq!(
        client.supports_capability(&Symbol::new(&env, "pause")),
        CapabilityStatus::Supported
    );
    assert_eq!(
        client.supports_capability(&Symbol::new(&env, "metadata")),
        CapabilityStatus::Supported
    );
    assert_eq!(
        client.supports_capability(&Symbol::new(&env, "events")),
        CapabilityStatus::Supported
    );
    assert_eq!(
        client.supports_capability(&Symbol::new(&env, "allowances")),
        CapabilityStatus::Planned
    );
    assert_eq!(
        client.supports_capability(&Symbol::new(&env, "burning")),
        CapabilityStatus::Unsupported
    );
}

#[test]
fn test_supports_capability_unknown_key_fails_safe() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // An unknown key — e.g. a newer SDK probing an older deployment — must
    // resolve to Unsupported rather than reverting the invocation.
    assert_eq!(
        client.supports_capability(&Symbol::new(&env, "not_a_capability")),
        CapabilityStatus::Unsupported
    );
    assert_eq!(
        client.supports_capability(&Symbol::new(&env, "staking")),
        CapabilityStatus::Unsupported
    );
}

#[test]
fn test_supports_capability_available_before_initialize_and_when_paused() {
    let (env, client, admin, _user1, _user2) = setup();

    // Before initialize — no auth mocked, no admin in storage.
    assert_eq!(
        client.supports_capability(&Symbol::new(&env, "whitelist")),
        CapabilityStatus::Supported
    );
    assert_eq!(client.get_capability_keys().len(), 27);

    env.mock_all_auths();
    client.initialize(&admin);
    client.pause(&admin);

    // And while paused.
    assert_eq!(
        client.supports_capability(&Symbol::new(&env, "whitelist")),
        CapabilityStatus::Supported
    );
    assert_eq!(client.get_capability_keys().len(), 27);
}

#[test]
fn test_capability_keys_all_resolve_and_match_descriptor() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    let keys = client.get_capability_keys();
    assert!(!keys.is_empty());

    // Every advertised key must resolve through the single-key helper, and
    // no advertised key may silently fall through to the unknown-key branch
    // unless it genuinely is Unsupported in the descriptor.
    let caps = client.get_capabilities();
    for key in keys.iter() {
        let status = client.supports_capability(&key);
        if key == Symbol::new(&env, "burning") {
            assert_eq!(status, caps.minting.burning);
        } else if key == Symbol::new(&env, "investor_tiers") {
            assert_eq!(status, caps.compliance.investor_tiers);
        } else if key == Symbol::new(&env, "transfer_restriction_events") {
            assert_eq!(status, caps.events.transfer_restriction_events);
        } else {
            // Everything else is a live or tracked capability.
            assert_ne!(
                status,
                CapabilityStatus::Unsupported,
                "advertised key resolved to Unsupported"
            );
        }
    }

    // Keys are unique — a duplicate would make client-side caching ambiguous.
    for (i, key) in keys.iter().enumerate() {
        for (j, other) in keys.iter().enumerate() {
            if i != j {
                assert_ne!(key, other, "duplicate capability key");
            }
        }
    }
}

#[test]
fn test_supports_capability_agrees_with_descriptor_across_state_changes() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user2);
    client.propose_holding_cap(&admin, &500);
    client.accept_holding_cap(&admin);
    client.mint_asset(&user1, &user2, &100);

    // Static capabilities must not drift as runtime state changes — that is
    // exactly the guarantee that lets clients cache them.
    let caps = client.get_capabilities();
    assert_eq!(
        client.supports_capability(&Symbol::new(&env, "holding_cap")),
        caps.transfers.holding_cap
    );
    assert_eq!(caps.transfers.holding_cap, CapabilityStatus::Supported);
    // ...while the runtime enforcement flag does track state.
    assert!(caps.transfers.holding_cap_enforced);
}
