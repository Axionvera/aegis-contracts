#![cfg(test)]

use super::*;
use crate::asset::{AssetMintedEvent, TransferEvent, YieldDistributedEvent};
use crate::compliance::{UserWhitelistedEvent, WhitelistRevokedEvent};
use crate::eligibility::InvestorEligibility;
use crate::errors::Error;
use crate::lifecycle::{AssetStatus, AssetStatusChangedEvent};
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    vec, Address, Env, IntoVal,
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

/// Like `setup()`, but also initializes the contract and transitions the asset
/// lifecycle to `Active` so mint/transfer calls are not blocked by lifecycle
/// guards. Use this in any test that performs a successful mint or transfer.
fn setup_active() -> (Env, AegisContractClient<'static>, Address, Address, Address) {
    let env = Env::default();
    let contract_id = env.register_contract(None, AegisContract);
    let client = AegisContractClient::new(&env, &contract_id);
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    client.initialize(&admin);
    client.set_asset_status(&admin, &AssetStatus::Active);
    (env, client, admin, user1, user2)
}

// ─── Happy-path lifecycle ─────────────────────────────────────────────────────

#[test]
fn test_lifecycle() {
    let (env, client, admin, user1, user2) = setup_active();
    env.mock_all_auths();

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
    let (env, client, admin, user1, user2) = setup_active();
    env.mock_all_auths();

    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user2);

    let result = client.try_mint_asset(&user1, &user2, &100);
    assert!(result.is_ok());
}

#[test]
fn test_mint_reverts_with_invalid_amount() {
    let (env, client, admin, user1, user2) = setup_active();
    env.mock_all_auths();

    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user2);

    let result = client.try_mint_asset(&user1, &user2, &0);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn test_mint_reverts_when_receiver_not_whitelisted() {
    let (env, client, admin, user1, user2) = setup_active();
    env.mock_all_auths();

    client.set_role(&admin, &user1, &Role::AssetManager);

    // user2 was never whitelisted
    let result = client.try_mint_asset(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::ReceiverNotWhitelisted)));
}

#[test]
fn test_mint_succeeds_with_admin_role() {
    let (env, client, admin, _user1, user2) = setup_active();
    env.mock_all_auths();

    client.whitelist_user(&admin, &user2);

    // Admin can mint without an explicit AssetManager role assignment
    let result = client.try_mint_asset(&admin, &user2, &100);
    assert!(result.is_ok());
}

// ─── Transfer validation ──────────────────────────────────────────────────────

#[test]
fn test_transfer_reverts_with_invalid_amount() {
    let (env, client, admin, user1, user2) = setup_active();
    env.mock_all_auths();

    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);

    let result = client.try_transfer(&user1, &user2, &0);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn test_transfer_reverts_when_sender_not_whitelisted() {
    let (env, client, admin, user1, user2) = setup_active();
    env.mock_all_auths();

    client.whitelist_user(&admin, &user2);

    // user1 was never whitelisted
    let result = client.try_transfer(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::SenderNotWhitelisted)));
}

#[test]
fn test_transfer_reverts_when_receiver_not_whitelisted() {
    let (env, client, admin, user1, user2) = setup_active();
    env.mock_all_auths();

    client.whitelist_user(&admin, &user1);

    // user2 was never whitelisted
    let result = client.try_transfer(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::ReceiverNotWhitelisted)));
}

#[test]
fn test_transfer_reverts_with_insufficient_balance() {
    let (env, client, admin, user1, user2) = setup_active();
    env.mock_all_auths();

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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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

// ─── Investor holding restriction checks (#33) ───────────────────────────────

#[test]
fn test_holding_cap_default_is_unrestricted() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
            asset_status: AssetStatus::Draft,
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
            asset_status: AssetStatus::Active,
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
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
    client.set_asset_status(&admin, &AssetStatus::Active);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user1);
    client.mint_asset(&user1, &user1, &1000);
    // user2 deliberately left off the whitelist.

    assert!(!client.check_transfer_eligibility(&user1, &user2, &100));
    let result = client.try_transfer(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::ReceiverNotWhitelisted)));
}

// ─── Asset lifecycle state machine ───────────────────────────────────────────
//
// Tests for the Draft → Active → Paused / Retired / Blocked state machine.
// Each test covers one concern: default state, valid transitions, invalid
// transitions, access control, operation gating per state, and event
// emission. The asset-lifecycle "Paused" state is intentionally distinct
// from the contract-level pause; both checks are validated independently.

// ── Default state ─────────────────────────────────────────────────────────────

#[test]
fn test_asset_status_defaults_to_draft() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // Before any explicit set_asset_status call the status must be Draft.
    assert_eq!(client.get_asset_status(), AssetStatus::Draft);
}

#[test]
fn test_get_asset_status_readable_without_initialization() {
    let (env, client, _admin, _user1, _user2) = setup();
    // Calling get_asset_status on a freshly registered (but not initialized)
    // contract must return Draft without panicking.
    assert_eq!(client.get_asset_status(), AssetStatus::Draft);
}

// ── Access control ────────────────────────────────────────────────────────────

#[test]
fn test_set_asset_status_reverts_for_non_admin() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // user1 has no role — transition must be rejected.
    let result = client.try_set_asset_status(&user1, &AssetStatus::Active);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
    assert_eq!(client.get_asset_status(), AssetStatus::Draft);
}

#[test]
fn test_set_asset_status_reverts_for_asset_manager() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);

    // AssetManager cannot manage lifecycle — only the supreme admin can.
    let result = client.try_set_asset_status(&user1, &AssetStatus::Active);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn test_set_asset_status_succeeds_for_admin() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    let result = client.try_set_asset_status(&admin, &AssetStatus::Active);
    assert!(result.is_ok());
    assert_eq!(client.get_asset_status(), AssetStatus::Active);
}

#[test]
fn test_set_asset_status_blocked_when_contract_paused() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.pause(&admin);

    // Transitioning TO Active is blocked while the contract is paused.
    let result = client.try_set_asset_status(&admin, &AssetStatus::Active);
    assert_eq!(result, Err(Ok(Error::ContractPaused)));
    // Status must remain Draft.
    assert_eq!(client.get_asset_status(), AssetStatus::Draft);
}

#[test]
fn test_set_asset_status_to_blocked_paused_retired_allowed_when_contract_paused() {
    // The admin should be able to lock down or retire an asset during an
    // incident without having to unpause the contract first.
    let (env, client, admin, _user1, _user2) = setup_active();
    env.mock_all_auths();

    client.pause(&admin);

    // Active → Blocked is allowed while the contract is paused.
    client.set_asset_status(&admin, &AssetStatus::Blocked);
    assert_eq!(client.get_asset_status(), AssetStatus::Blocked);

    // Blocked → Active is NOT allowed (contract still paused).
    let result = client.try_set_asset_status(&admin, &AssetStatus::Active);
    assert_eq!(result, Err(Ok(Error::ContractPaused)));

    // Unpause so we can set up the Paused → Blocked → Active path below.
    client.unpause(&admin);

    // Reset to Active so we can test the Paused (lifecycle) path.
    client.set_asset_status(&admin, &AssetStatus::Active);
    client.pause(&admin);

    // Active → Paused (lifecycle) is allowed while the contract is paused.
    client.set_asset_status(&admin, &AssetStatus::Paused);
    assert_eq!(client.get_asset_status(), AssetStatus::Paused);

    // Paused (lifecycle) → Retired is also allowed while the contract is paused.
    client.set_asset_status(&admin, &AssetStatus::Retired);
    assert_eq!(client.get_asset_status(), AssetStatus::Retired);
}

// ── Valid transitions ─────────────────────────────────────────────────────────

#[test]
fn test_transition_draft_to_active() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    assert_eq!(client.get_asset_status(), AssetStatus::Draft);

    client.set_asset_status(&admin, &AssetStatus::Active);
    assert_eq!(client.get_asset_status(), AssetStatus::Active);
}

#[test]
fn test_transition_active_to_paused() {
    let (env, client, admin, _user1, _user2) = setup_active();
    env.mock_all_auths();

    client.set_asset_status(&admin, &AssetStatus::Paused);
    assert_eq!(client.get_asset_status(), AssetStatus::Paused);
}

#[test]
fn test_transition_active_to_retired() {
    let (env, client, admin, _user1, _user2) = setup_active();
    env.mock_all_auths();

    client.set_asset_status(&admin, &AssetStatus::Retired);
    assert_eq!(client.get_asset_status(), AssetStatus::Retired);
}

#[test]
fn test_transition_active_to_blocked() {
    let (env, client, admin, _user1, _user2) = setup_active();
    env.mock_all_auths();

    client.set_asset_status(&admin, &AssetStatus::Blocked);
    assert_eq!(client.get_asset_status(), AssetStatus::Blocked);
}

#[test]
fn test_transition_paused_to_active() {
    let (env, client, admin, _user1, _user2) = setup_active();
    env.mock_all_auths();

    client.set_asset_status(&admin, &AssetStatus::Paused);
    client.set_asset_status(&admin, &AssetStatus::Active);
    assert_eq!(client.get_asset_status(), AssetStatus::Active);
}

#[test]
fn test_transition_paused_to_retired() {
    let (env, client, admin, _user1, _user2) = setup_active();
    env.mock_all_auths();

    client.set_asset_status(&admin, &AssetStatus::Paused);
    client.set_asset_status(&admin, &AssetStatus::Retired);
    assert_eq!(client.get_asset_status(), AssetStatus::Retired);
}

#[test]
fn test_transition_paused_to_blocked() {
    let (env, client, admin, _user1, _user2) = setup_active();
    env.mock_all_auths();

    client.set_asset_status(&admin, &AssetStatus::Paused);
    client.set_asset_status(&admin, &AssetStatus::Blocked);
    assert_eq!(client.get_asset_status(), AssetStatus::Blocked);
}

#[test]
fn test_transition_blocked_to_active() {
    let (env, client, admin, _user1, _user2) = setup_active();
    env.mock_all_auths();

    client.set_asset_status(&admin, &AssetStatus::Blocked);
    client.set_asset_status(&admin, &AssetStatus::Active);
    assert_eq!(client.get_asset_status(), AssetStatus::Active);
}

// ── Invalid / rejected transitions ───────────────────────────────────────────

#[test]
fn test_transition_noop_same_status_rejected() {
    let (env, client, admin, _user1, _user2) = setup_active();
    env.mock_all_auths();

    // Setting the current status to itself is a no-op and must be rejected.
    let result = client.try_set_asset_status(&admin, &AssetStatus::Active);
    assert_eq!(result, Err(Ok(Error::InvalidLifecycleTransition)));
    // Status must not change.
    assert_eq!(client.get_asset_status(), AssetStatus::Active);
}

#[test]
fn test_transition_draft_to_paused_rejected() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    let result = client.try_set_asset_status(&admin, &AssetStatus::Paused);
    assert_eq!(result, Err(Ok(Error::InvalidLifecycleTransition)));
    assert_eq!(client.get_asset_status(), AssetStatus::Draft);
}

#[test]
fn test_transition_draft_to_retired_rejected() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    let result = client.try_set_asset_status(&admin, &AssetStatus::Retired);
    assert_eq!(result, Err(Ok(Error::InvalidLifecycleTransition)));
    assert_eq!(client.get_asset_status(), AssetStatus::Draft);
}

#[test]
fn test_transition_draft_to_blocked_rejected() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    let result = client.try_set_asset_status(&admin, &AssetStatus::Blocked);
    assert_eq!(result, Err(Ok(Error::InvalidLifecycleTransition)));
    assert_eq!(client.get_asset_status(), AssetStatus::Draft);
}

#[test]
fn test_transition_blocked_to_paused_rejected() {
    let (env, client, admin, _user1, _user2) = setup_active();
    env.mock_all_auths();

    client.set_asset_status(&admin, &AssetStatus::Blocked);

    let result = client.try_set_asset_status(&admin, &AssetStatus::Paused);
    assert_eq!(result, Err(Ok(Error::InvalidLifecycleTransition)));
    assert_eq!(client.get_asset_status(), AssetStatus::Blocked);
}

#[test]
fn test_transition_blocked_to_retired_rejected() {
    let (env, client, admin, _user1, _user2) = setup_active();
    env.mock_all_auths();

    client.set_asset_status(&admin, &AssetStatus::Blocked);

    let result = client.try_set_asset_status(&admin, &AssetStatus::Retired);
    assert_eq!(result, Err(Ok(Error::InvalidLifecycleTransition)));
    assert_eq!(client.get_asset_status(), AssetStatus::Blocked);
}

#[test]
fn test_retired_is_terminal_no_transition_out() {
    let (env, client, admin, _user1, _user2) = setup_active();
    env.mock_all_auths();

    client.set_asset_status(&admin, &AssetStatus::Retired);

    // Every possible target state must be rejected.
    for next in &[
        AssetStatus::Draft,
        AssetStatus::Active,
        AssetStatus::Paused,
        AssetStatus::Retired, // no-op / same state
        AssetStatus::Blocked,
    ] {
        let result = client.try_set_asset_status(&admin, next);
        assert_eq!(
            result,
            Err(Ok(Error::InvalidLifecycleTransition)),
            "Expected InvalidLifecycleTransition when transitioning from Retired to {:?}",
            next
        );
    }
    assert_eq!(client.get_asset_status(), AssetStatus::Retired);
}

// ── Operation gating per lifecycle state ─────────────────────────────────────

#[test]
fn test_mint_blocked_in_draft_state() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    // Status is Draft by default — no activation.
    client.whitelist_user(&admin, &user2);

    let result = client.try_mint_asset(&admin, &user2, &100);
    assert_eq!(result, Err(Ok(Error::AssetNotActive)));
}

#[test]
fn test_transfer_blocked_in_draft_state() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    // Status is Draft — transfers should be blocked.
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);

    let result = client.try_transfer(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::AssetNotActive)));
}

#[test]
fn test_mint_blocked_in_lifecycle_paused_state() {
    let (env, client, admin, _user1, user2) = setup_active();
    env.mock_all_auths();

    client.whitelist_user(&admin, &user2);
    // Confirm mint works before pausing lifecycle.
    assert!(client.try_mint_asset(&admin, &user2, &50).is_ok());

    client.set_asset_status(&admin, &AssetStatus::Paused);

    let result = client.try_mint_asset(&admin, &user2, &50);
    assert_eq!(result, Err(Ok(Error::AssetLifecyclePaused)));
}

#[test]
fn test_transfer_blocked_in_lifecycle_paused_state() {
    let (env, client, admin, user1, user2) = setup_active();
    env.mock_all_auths();

    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&admin, &user1, &500);

    // Confirm transfer works before pausing lifecycle.
    assert!(client.try_transfer(&user1, &user2, &100).is_ok());

    client.set_asset_status(&admin, &AssetStatus::Paused);

    let result = client.try_transfer(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::AssetLifecyclePaused)));
}

#[test]
fn test_mint_blocked_in_retired_state() {
    let (env, client, admin, _user1, user2) = setup_active();
    env.mock_all_auths();

    client.whitelist_user(&admin, &user2);
    client.set_asset_status(&admin, &AssetStatus::Retired);

    let result = client.try_mint_asset(&admin, &user2, &100);
    assert_eq!(result, Err(Ok(Error::AssetRetired)));
}

#[test]
fn test_transfer_blocked_in_retired_state() {
    let (env, client, admin, user1, user2) = setup_active();
    env.mock_all_auths();

    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&admin, &user1, &500);
    client.set_asset_status(&admin, &AssetStatus::Retired);

    let result = client.try_transfer(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::AssetRetired)));
}

#[test]
fn test_mint_blocked_in_blocked_state() {
    let (env, client, admin, _user1, user2) = setup_active();
    env.mock_all_auths();

    client.whitelist_user(&admin, &user2);
    client.set_asset_status(&admin, &AssetStatus::Blocked);

    let result = client.try_mint_asset(&admin, &user2, &100);
    assert_eq!(result, Err(Ok(Error::AssetBlocked)));
}

#[test]
fn test_transfer_blocked_in_blocked_state() {
    let (env, client, admin, user1, user2) = setup_active();
    env.mock_all_auths();

    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&admin, &user1, &500);
    client.set_asset_status(&admin, &AssetStatus::Blocked);

    let result = client.try_transfer(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::AssetBlocked)));
}

#[test]
fn test_mint_succeeds_after_reactivation_from_paused() {
    let (env, client, admin, _user1, user2) = setup_active();
    env.mock_all_auths();

    client.whitelist_user(&admin, &user2);
    client.set_asset_status(&admin, &AssetStatus::Paused);

    // Still blocked in Paused.
    assert_eq!(
        client.try_mint_asset(&admin, &user2, &100),
        Err(Ok(Error::AssetLifecyclePaused))
    );

    // After reactivation, mint should work again.
    client.set_asset_status(&admin, &AssetStatus::Active);
    assert!(client.try_mint_asset(&admin, &user2, &100).is_ok());
    assert_eq!(client.get_balance_of(&user2), 100);
}

#[test]
fn test_transfer_succeeds_after_reactivation_from_blocked() {
    let (env, client, admin, user1, user2) = setup_active();
    env.mock_all_auths();

    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&admin, &user1, &500);

    client.set_asset_status(&admin, &AssetStatus::Blocked);

    // Still blocked.
    assert_eq!(
        client.try_transfer(&user1, &user2, &100),
        Err(Ok(Error::AssetBlocked))
    );

    // Unblock → transfer works again.
    client.set_asset_status(&admin, &AssetStatus::Active);
    assert!(client.try_transfer(&user1, &user2, &100).is_ok());
    assert_eq!(client.get_balance_of(&user2), 100);
}

// ── Independent dual-pause checks ────────────────────────────────────────────

#[test]
fn test_contract_pause_and_lifecycle_pause_are_independent() {
    // Verify that both must be clear for mint to succeed, and that each
    // can block independently. The two checks are orthogonal.
    let (env, client, admin, _user1, user2) = setup_active();
    env.mock_all_auths();

    client.whitelist_user(&admin, &user2);

    // Contract paused only → ContractPaused error.
    client.pause(&admin);
    assert_eq!(
        client.try_mint_asset(&admin, &user2, &100),
        Err(Ok(Error::ContractPaused))
    );

    // Unpause contract — should now work (lifecycle is still Active).
    client.unpause(&admin);
    assert!(client.try_mint_asset(&admin, &user2, &10).is_ok());

    // Lifecycle paused only → AssetLifecyclePaused error.
    client.set_asset_status(&admin, &AssetStatus::Paused);
    assert_eq!(
        client.try_mint_asset(&admin, &user2, &100),
        Err(Ok(Error::AssetLifecyclePaused))
    );

    // Both paused simultaneously → ContractPaused wins (checked first).
    client.pause(&admin);
    assert_eq!(
        client.try_mint_asset(&admin, &user2, &100),
        Err(Ok(Error::ContractPaused))
    );
}

#[test]
fn test_lifecycle_paused_does_not_affect_contract_pause_state() {
    // Pausing the asset lifecycle must not alter the contract-level pause flag.
    let (env, client, admin, _user1, _user2) = setup_active();
    env.mock_all_auths();

    assert!(!client.is_paused());
    client.set_asset_status(&admin, &AssetStatus::Paused);
    // Contract-level pause flag must still be false.
    assert!(!client.is_paused());
    assert_eq!(client.get_asset_status(), AssetStatus::Paused);
}

// ── Read-only functions unaffected by lifecycle state ────────────────────────

#[test]
fn test_get_asset_status_readable_in_all_states() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // Draft
    assert_eq!(client.get_asset_status(), AssetStatus::Draft);

    // Active
    client.set_asset_status(&admin, &AssetStatus::Active);
    assert_eq!(client.get_asset_status(), AssetStatus::Active);

    // Paused
    client.set_asset_status(&admin, &AssetStatus::Paused);
    assert_eq!(client.get_asset_status(), AssetStatus::Paused);

    // Blocked
    client.set_asset_status(&admin, &AssetStatus::Blocked);
    assert_eq!(client.get_asset_status(), AssetStatus::Blocked);

    // Active again
    client.set_asset_status(&admin, &AssetStatus::Active);

    // Retired (terminal)
    client.set_asset_status(&admin, &AssetStatus::Retired);
    assert_eq!(client.get_asset_status(), AssetStatus::Retired);
}

// ── Event emission ────────────────────────────────────────────────────────────

#[test]
fn test_set_asset_status_emits_event() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_asset_status(&admin, &AssetStatus::Active);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("asset_status_changed",).into_val(&env),
                AssetStatusChangedEvent {
                    admin: admin.clone(),
                    previous_status: AssetStatus::Draft,
                    new_status: AssetStatus::Active,
                }
                .into_val(&env),
            ),
        ]
    );
}

#[test]
fn test_set_asset_status_emits_event_on_each_valid_transition() {
    let (env, client, admin, _user1, _user2) = setup_active();
    env.mock_all_auths();

    // Active → Blocked
    client.set_asset_status(&admin, &AssetStatus::Blocked);
    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("asset_status_changed",).into_val(&env),
                AssetStatusChangedEvent {
                    admin: admin.clone(),
                    previous_status: AssetStatus::Active,
                    new_status: AssetStatus::Blocked,
                }
                .into_val(&env),
            ),
        ]
    );

    // Blocked → Active
    client.set_asset_status(&admin, &AssetStatus::Active);
    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("asset_status_changed",).into_val(&env),
                AssetStatusChangedEvent {
                    admin: admin.clone(),
                    previous_status: AssetStatus::Blocked,
                    new_status: AssetStatus::Active,
                }
                .into_val(&env),
            ),
        ]
    );
}

#[test]
fn test_invalid_lifecycle_transition_emits_no_event() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    // Draft → Retired is invalid; no event must be emitted.
    let result = client.try_set_asset_status(&admin, &AssetStatus::Retired);
    assert_eq!(result, Err(Ok(Error::InvalidLifecycleTransition)));
    assert_eq!(env.events().all().events().len(), 0);
}
