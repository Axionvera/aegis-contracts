

#![cfg(test)]


use super::*;
use crate::asset::{AssetMintedEvent, AssetStatus, TransferEvent, YieldDistributedEvent};

use crate::admin::{
    AdminTransferredEvent, AdminTransferInitiatedEvent, ContractPausedEvent,
    ContractUnpausedEvent, RoleAssignedEvent, RoleRevokedEvent,
};
use crate::capabilities::{
    CapabilityStatus, ComplianceCapabilities, ContractCapabilities, EventCapabilities,
    MetadataCapabilities, MintingCapabilities, PauseCapabilities, TransferCapabilities,
    CAPABILITY_SCHEMA_VERSION,
};

use crate::compliance::{UserWhitelistedEvent, WhitelistRevokedEvent};
use crate::errors::Error;
use crate::ContractInitializedEvent;
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    vec, Address, Env, IntoVal, String,
};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn setup() -> (Env, AegisContractClient<'static>, Address, Address, Address) {
    let env = Env::default();
    let contract_id = env.register(AegisContract, ());
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

// ─── Event compatibility: admin, role & governance ────────────────────────────
//
// These tests lock down the exact topic and payload shape of every
// role-related and admin-related event so downstream audit-trail UIs,
// dashboards, and indexers get a compile-time-checked regression signal if
// the schema ever drifts.

#[test]
fn test_initialize_emits_event() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("contract_initialized",).into_val(&env),
                ContractInitializedEvent { admin }
                    .into_val(&env),
            ),
        ]
    );
}

#[test]
fn test_set_role_emits_event() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::ComplianceOfficer);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("role_assigned",).into_val(&env),
                RoleAssignedEvent {
                    admin: admin.clone(),
                    target: user1,
                    role: Role::ComplianceOfficer,
                }
                .into_val(&env),
            ),
        ]
    );
}

#[test]
fn test_remove_role_emits_event() {
    let (env, client, admin, user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.remove_role(&admin, &user1);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("role_revoked",).into_val(&env),
                RoleRevokedEvent {
                    admin: admin.clone(),
                    target: user1,
                    role: Role::AssetManager,
                }
                .into_val(&env),
            ),
        ]
    );
}

#[test]
fn test_transfer_admin_emits_event() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.transfer_admin(&admin, &user2);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("admin_transfer_initiated",).into_val(&env),
                AdminTransferInitiatedEvent {
                    current_admin: admin,
                    candidate: user2,
                }
                .into_val(&env),
            ),
        ]
    );
}

#[test]
fn test_accept_admin_emits_event() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.transfer_admin(&admin, &user2);
    client.accept_admin(&user2);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("admin_transferred",).into_val(&env),
                AdminTransferredEvent {
                    previous_admin: admin,
                    new_admin: user2,
                }
                .into_val(&env),
            ),
        ]
    );
}

#[test]
fn test_renounce_admin_emits_event() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.renounce_admin(&admin);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("admin_renounced",).into_val(&env),
                AdminTransferredEvent {
                    previous_admin: admin.clone(),
                    new_admin: admin,
                }
                .into_val(&env),
            ),
        ]
    );
}

#[test]
fn test_pause_emits_event() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.pause(&admin);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("contract_paused",).into_val(&env),
                ContractPausedEvent { admin }
                    .into_val(&env),
            ),
        ]
    );
}

#[test]
fn test_unpause_emits_event() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.pause(&admin);
    client.unpause(&admin);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("contract_unpaused",).into_val(&env),
                ContractUnpausedEvent { admin }
                    .into_val(&env),
            ),
        ]
    );
}

// ─── Supply cap amendment governance (#32) ────────────────────────────────────

#[test]
fn test_supply_cap_default_is_unbounded() {
    let (env, client, admin, _user1, user2) = setup();
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


    // ── Pre-state snapshot (baseline) ───────────────────────────────────────
    let initial_total_supply = client.get_total_supply();
    let initial_balance_u1 = client.get_balance_of(&user1);
    let initial_balance_u2 = client.get_balance_of(&user2);
    let initial_role_u1 = client.get_role_of(&user1);
    let initial_whitelist_u1 = client.is_whitelisted(&user1);
    let initial_asset_status = client.get_asset_status();

    // ── 1. CONFIG / INITIALIZATION ──────────────────────────────────────────

    // Initialize the fresh deployment first, then re-assert the double-init
    // guard for both the admin and an unrelated caller.
    client.initialize(&admin);

    // First init to establish the admin.
    client.initialize(&admin);

    // Double init (already covered but re-assert in matrix)

    let r = client.try_initialize(&admin);
    assert_eq!(r, Err(Ok(Error::AlreadyInitialized)));

    let r = client.try_initialize(&user1);
    assert_eq!(r, Err(Ok(Error::AlreadyInitialized)));

    // ── 2. ROLE MANAGEMENT INVALID INPUTS ───────────────────────────────────
    // Non-admin caller
    let r = client.try_set_role(&user1, &user2, &Role::AssetManager);
    assert_eq!(r, Err(Ok(Error::Unauthorized)));

    // Cannot assign Admin via set_role
    let r = client.try_set_role(&admin, &user2, &Role::Admin);
    assert_eq!(r, Err(Ok(Error::CannotAssignAdminRole)));

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


    // Supply cap governance is blocked while paused.
    let r = client.try_propose_supply_cap(&admin, &-1);
    assert!(r.is_err());
    let r = client.try_propose_supply_cap(&admin, &0);
    assert!(r.is_err());

    // Holding cap governance is also blocked while paused.
    let r = client.try_propose_holding_cap(&admin, &-1);

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


    // While paused AND retired, even fully whitelisted parties cannot
    // transfer; the point-in-time eligibility helper agrees.
    assert!(!client.check_transfer_eligibility(&user1, &user2, &100));
    let r = client.try_transfer(&user1, &user2, &100);
    assert_eq!(r, Err(Ok(Error::ContractPaused)));

    // Unpause when not paused (after unpause)
    client.unpause(&admin);
    let r = client.try_unpause(&admin);
    assert_eq!(r, Err(Ok(Error::NotPaused)));

    // Cap validation now fires for the right reason: negative values and
    // no-op proposals are rejected.
    let r = client.try_propose_supply_cap(&admin, &-1);
    assert!(r.is_err());
    let r = client.try_propose_supply_cap(&admin, &0);
    assert!(r.is_err());
    let r = client.try_propose_holding_cap(&admin, &-1);
    assert!(r.is_err());

    // Retired is terminal: minting and transfers stay blocked even after the
    // pause is lifted, even between whitelisted parties.
    let r = client.try_mint_asset(&user1, &user2, &100);
    assert_eq!(r, Err(Ok(Error::AssetNotActive)));
    let r = client.try_transfer(&user1, &user2, &100);
    assert_eq!(r, Err(Ok(Error::AssetNotActive)));

    // ── FINAL STATE VERIFICATION: NO MUTATION FROM REJECTED INPUTS ──────────
    // Sanity-check the captured baseline (fresh, pre-initialization deploy).
    assert_eq!(initial_total_supply, 0);
    assert_eq!(initial_balance_u1, 0);
    assert_eq!(initial_balance_u2, 0);
    assert_eq!(initial_role_u1, Role::None);
    assert!(!initial_whitelist_u1);
    assert_eq!(initial_asset_status, AssetStatus::Active);

    // Only the deliberate, valid operations above are visible in state;
    // every rejected invalid input left no trace.
    assert_eq!(client.get_total_supply(), initial_total_supply + 500);
    assert_eq!(client.get_balance_of(&user1), initial_balance_u1 + 500);
    assert_eq!(client.get_balance_of(&user2), initial_balance_u2);
    assert_eq!(client.get_role_of(&user1), Role::AssetManager);
    assert!(client.is_whitelisted(&user1));
    assert!(client.is_whitelisted(&user2));
    assert_eq!(client.get_asset_status(), AssetStatus::Retired);
}


    // ── 7. ELIGIBILITY & FINAL STATE VERIFICATION ──────────────────────────
    // Contract is paused → transfer eligibility must report false.
    assert!(!client.check_transfer_eligibility(&user1, &user2, &100));


    // Unpause to verify final transfer behaviour with Retired asset.
    client.unpause(&admin);

    // After unpause, check_transfer_eligibility does not check asset status —
    // it only covers pause, whitelist, holding cap, and balance. So it now
    // returns true for a whitelisted pair with sufficient balance.
    assert!(client.check_transfer_eligibility(&user1, &user2, &100));

    // But the actual transfer still fails because the asset is Retired.
    let result = client.try_transfer(&user1, &user2, &100);
    assert_eq!(result, Err(Ok(Error::AssetNotActive)));

    // ── FINAL STATE VERIFICATION: NO UNEXPECTED MUTATION ───────────────────
    assert_eq!(client.get_total_supply(), initial_total_supply + 500);
    assert_eq!(client.get_balance_of(&user1), initial_balance_u1 + 500);
    assert_eq!(client.get_balance_of(&user2), initial_balance_u2);
    assert_eq!(client.get_role_of(&user1), Role::AssetManager);
    assert!(client.is_whitelisted(&user1));
    assert_eq!(client.get_asset_status(), AssetStatus::Retired);

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

// ─── Supply cap error standardization (#32) ────────────────────────────────────

#[test]
fn test_supply_cap_exceeded_returns_standardized_error() {
    let (env, client, admin, _user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.whitelist_user(&admin, &user2);
    client.propose_supply_cap(&admin, &100);
    client.accept_supply_cap(&admin);

    // Reach the cap exactly.
    client.mint_asset(&admin, &user2, &100);
    assert_eq!(client.get_total_supply(), 100);

    // Any mint beyond cap must return the standardized error code.
    let r = client.try_mint_asset(&admin, &user2, &1);
    assert_eq!(r, Err(Ok(Error::SupplyCapExceeded)));
}

#[test]
fn test_holding_cap_exceeded_returns_standardized_error() {
    let (env, client, admin, user1, user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user1);
    client.whitelist_user(&admin, &user2);
    client.mint_asset(&user1, &user1, &500);
    client.propose_holding_cap(&admin, &300);
    client.accept_holding_cap(&admin);

    // Transfer that pushes receiver over cap must return the standardized error.
    let r = client.try_transfer(&user1, &user2, &301);
    assert_eq!(r, Err(Ok(Error::HoldingCapExceeded)));
}

// ─── Supply cap event compatibility (#32) ─────────────────────────────────────

#[test]
fn test_supply_cap_proposed_and_amended_emit_events() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.propose_supply_cap(&admin, &500);

    // Propose event
    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("supply_cap_proposed",).into_val(&env),
                crate::supply_cap::SupplyCapProposedEvent {
                    admin,
                    current_cap: 0,
                    proposed_cap: 500,
                }
                .into_val(&env),
            ),
        ]
    );

    client.accept_supply_cap(&admin);

    // Amended event
    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("supply_cap_amended",).into_val(&env),
                crate::supply_cap::SupplyCapAmendedEvent {
                    admin,
                    previous_cap: 0,
                    new_cap: 500,
                }
                .into_val(&env),
            ),
        ]
    );
}


// ─── COMPLIANCE STATUS TRANSITION INVARIANTS (Audit Readiness) ────────────────
//
// Deterministic invariant coverage for compliance status transitions. The
// full model — statuses, transition matrix, authorization rules, the blocked
// overlay, event guarantees, and consistency invariants — is documented in
// docs/compliance-status-transitions.md.
//
// The compliance registry is an address-keyed set (see
// docs/compliance-registry-reads.md), so an investor's *compliance status* is
// derived from observable contract state plus the transition history of the
// address:
//
// | Status    | How the address got there                          | is_whitelisted |
// |-----------|----------------------------------------------------|----------------|
// | Unknown   | never targeted by any compliance call              | false          |
// | Pending   | an approval attempt was rejected (still waiting)   | false          |
// | Approved  | a committed `whitelist_user`                       | true           |
// | Revoked   | a committed `revoke_whitelist` after Approved      | false          |
//
// "Blocked" is a global overlay rather than a per-address status: while the
// contract is paused, EVERY compliance transition attempt reverts with
// `ContractPaused`, regardless of address status or caller.
//
// Every matrix row runs against a fresh contract deployment, so results
// depend only on the row's (status, action, caller) triple — never on test
// execution order.

/// The compliance statuses an investor address can be in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComplianceStatus {
    /// No record: the address has never been targeted by a compliance call.
    Unknown,
    /// Awaiting approval: an earlier approval attempt was rejected (wrong
    /// caller or paused contract), so the address is still off the whitelist.
    Pending,
    /// On the whitelist: may receive and send assets.
    Approved,
    /// Removed from the whitelist after having been approved.
    Revoked,
}

impl ComplianceStatus {
    const ALL: [ComplianceStatus; 4] = [
        ComplianceStatus::Unknown,
        ComplianceStatus::Pending,
        ComplianceStatus::Approved,
        ComplianceStatus::Revoked,
    ];

    /// The whitelist flag observable on-chain for an address in this status.
    fn is_approved(self) -> bool {
        matches!(self, ComplianceStatus::Approved)
    }
}

/// The two compliance transitions the contract exposes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComplianceAction {
    /// `whitelist_user` — drives an address towards `Approved`.
    Approve,
    /// `revoke_whitelist` — drives an address towards `Revoked`.
    Revoke,
}

impl ComplianceAction {
    const ALL: [ComplianceAction; 2] = [ComplianceAction::Approve, ComplianceAction::Revoke];

    /// The whitelist flag this action commits on success.
    fn result_is_approved(self) -> bool {
        matches!(self, ComplianceAction::Approve)
    }
}

/// Caller classes exercised against the transition matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransitionCaller {
    /// The supreme admin — bypasses role checks.
    Admin,
    /// Compliance-scoped role — manages the whitelist.
    ComplianceOfficer,
    /// Combined compliance + asset role — also manages the whitelist.
    EmergencyOfficer,
    /// No role at all — must always be rejected.
    NoRole,
    /// A real but wrong-scoped role — must always be rejected.
    AssetManager,
}

impl TransitionCaller {
    const ALL: [TransitionCaller; 5] = [
        TransitionCaller::Admin,
        TransitionCaller::ComplianceOfficer,
        TransitionCaller::EmergencyOfficer,
        TransitionCaller::NoRole,
        TransitionCaller::AssetManager,
    ];

    fn is_authorised(self) -> bool {
        matches!(
            self,
            TransitionCaller::Admin
                | TransitionCaller::ComplianceOfficer
                | TransitionCaller::EmergencyOfficer
        )
    }
}

/// Normalized result of a compliance transition attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransitionOutcome {
    /// The transition committed; state and events were written.
    Committed,
    /// The contract rejected the transition with a standardized error code.
    Rejected(Error),
    /// Any other abort (host error) — never expected in these tests.
    Aborted,
}

/// A fresh deployment wired with the named actors the matrix needs.
struct ComplianceFixture {
    env: Env,
    client: AegisContractClient<'static>,
    admin: Address,
    officer: Address,
    emergency: Address,
    manager: Address,
    intruder: Address,
    target: Address,
}

impl ComplianceFixture {
    fn new() -> Self {
        let env = Env::default();
        let contract_id = env.register(AegisContract, ());
        let client = AegisContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let officer = Address::generate(&env);
        let emergency = Address::generate(&env);
        let manager = Address::generate(&env);
        let intruder = Address::generate(&env);
        let target = Address::generate(&env);

        env.mock_all_auths();
        client.initialize(&admin);
        client.set_role(&admin, &officer, &Role::ComplianceOfficer);
        client.set_role(&admin, &emergency, &Role::EmergencyOfficer);
        client.set_role(&admin, &manager, &Role::AssetManager);

        Self {
            env,
            client,
            admin,
            officer,
            emergency,
            manager,
            intruder,
            target,
        }
    }

    fn caller_address(&self, caller: TransitionCaller) -> Address {
        match caller {
            TransitionCaller::Admin => self.admin.clone(),
            TransitionCaller::ComplianceOfficer => self.officer.clone(),
            TransitionCaller::EmergencyOfficer => self.emergency.clone(),
            TransitionCaller::NoRole => self.intruder.clone(),
            TransitionCaller::AssetManager => self.manager.clone(),
        }
    }

    fn is_target_approved(&self) -> bool {
        self.client.is_whitelisted(&self.target)
    }

    /// Events observed from the most recent top-level invocation.
    fn last_invocation_event_count(&self) -> usize {
        self.env.events().all().events().len()
    }

    /// Drives `target` into `status` using committed transitions — plus, for
    /// `Pending`, one rejected approval attempt by an unauthorised caller.
    fn drive_to_status(&self, status: ComplianceStatus) {
        match status {
            ComplianceStatus::Unknown => {}
            ComplianceStatus::Pending => {
                let outcome = self.attempt(ComplianceAction::Approve, TransitionCaller::NoRole);
                assert_eq!(outcome, TransitionOutcome::Rejected(Error::Unauthorized));
            }
            ComplianceStatus::Approved => {
                self.client.whitelist_user(&self.officer, &self.target);
            }
            ComplianceStatus::Revoked => {
                self.client.whitelist_user(&self.officer, &self.target);
                self.client.revoke_whitelist(&self.officer, &self.target);
            }
        }
        assert_eq!(
            self.is_target_approved(),
            status.is_approved(),
            "fixture failed to reach {status:?}"
        );
    }

    /// Attempts `action` on `target` as `caller`, normalizing the result.
    fn attempt(&self, action: ComplianceAction, caller: TransitionCaller) -> TransitionOutcome {
        let caller = self.caller_address(caller);
        let result = match action {
            ComplianceAction::Approve => self.client.try_whitelist_user(&caller, &self.target),
            ComplianceAction::Revoke => self.client.try_revoke_whitelist(&caller, &self.target),
        };
        match result {
            Ok(Ok(())) => TransitionOutcome::Committed,
            Err(Ok(e)) => TransitionOutcome::Rejected(e),
            _ => TransitionOutcome::Aborted,
        }
    }
}

#[test]
fn test_compliance_transition_matrix_deterministic() {
    // The deterministic transition matrix: every (status, action, caller)
    // combination against a fresh deployment.
    //
    // Authorised callers: every transition commits and the final status is
    // exactly the action's target — Approve always lands on Approved
    // (idempotent re-approval) and Revoke always lands off the whitelist
    // (idempotent no-op for Unknown/Pending/Revoked).
    //
    // Unauthorised callers: every transition is rejected with `Unauthorized`
    // and the address status is left exactly as it was — invalid transitions
    // cannot create a bypass.
    for status in ComplianceStatus::ALL {
        for action in ComplianceAction::ALL {
            for caller in TransitionCaller::ALL {
                let fixture = ComplianceFixture::new();
                fixture.drive_to_status(status);
                let before = fixture.is_target_approved();

                let outcome = fixture.attempt(action, caller);

                if caller.is_authorised() {
                    assert_eq!(
                        outcome,
                        TransitionOutcome::Committed,
                        "{status:?} + {action:?} by {caller:?} must commit"
                    );
                    assert_eq!(
                        fixture.is_target_approved(),
                        action.result_is_approved(),
                        "wrong final status: {status:?} + {action:?} by {caller:?}"
                    );
                } else {
                    assert_eq!(
                        outcome,
                        TransitionOutcome::Rejected(Error::Unauthorized),
                        "{status:?} + {action:?} by {caller:?} must be Unauthorized"
                    );
                    assert_eq!(
                        fixture.is_target_approved(),
                        before,
                        "unauthorised {action:?} changed status {status:?} for {caller:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn test_compliance_transitions_blocked_when_paused() {
    // The blocked overlay: while paused, EVERY transition attempt — from any
    // status, with any action, by any caller, authorised or not — reverts
    // with `ContractPaused`, because the pause guard runs before role checks.
    // No event escapes a blocked transition and the address status is left
    // unchanged. After unpause, authorised transitions work again, so the
    // blocked overlay never becomes a permanent lockout.
    for status in ComplianceStatus::ALL {
        for action in ComplianceAction::ALL {
            for caller in TransitionCaller::ALL {
                let fixture = ComplianceFixture::new();
                fixture.drive_to_status(status);
                fixture.client.pause(&fixture.admin);
                let before = fixture.is_target_approved();

                let outcome = fixture.attempt(action, caller);

                assert_eq!(
                    outcome,
                    TransitionOutcome::Rejected(Error::ContractPaused),
                    "blocked {action:?} from {status:?} by {caller:?} must report ContractPaused"
                );
                assert_eq!(
                    fixture.is_target_approved(),
                    before,
                    "blocked {action:?} changed status {status:?} for {caller:?}"
                );
                assert_eq!(
                    fixture.last_invocation_event_count(),
                    0,
                    "blocked {action:?} from {status:?} by {caller:?} emitted an event"
                );

                // Unpause: the same transition now resolves by caller class.
                fixture.client.unpause(&fixture.admin);
                let outcome = fixture.attempt(action, caller);
                if caller.is_authorised() {
                    assert_eq!(
                        outcome,
                        TransitionOutcome::Committed,
                        "post-unpause {action:?} by {caller:?} must commit"
                    );
                    assert_eq!(
                        fixture.is_target_approved(),
                        action.result_is_approved(),
                        "post-unpause final status wrong: {status:?} + {action:?} by {caller:?}"
                    );
                } else {
                    assert_eq!(
                        outcome,
                        TransitionOutcome::Rejected(Error::Unauthorized),
                        "post-unpause {action:?} by {caller:?} must be Unauthorized"
                    );
                    assert_eq!(
                        fixture.is_target_approved(),
                        before,
                        "post-unpause unauthorised {action:?} changed state for {caller:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn test_compliance_transition_events_have_exact_shape() {
    let fixture = ComplianceFixture::new();

    // Approve by a role caller (not the admin): the `caller` field must
    // record the officer, proving the event tracks the actual authorizer.
    fixture
        .client
        .whitelist_user(&fixture.officer, &fixture.target);
    assert_eq!(
        fixture.env.events().all(),
        vec![
            &fixture.env,
            (
                fixture.client.address.clone(),
                ("user_whitelisted",).into_val(&fixture.env),
                UserWhitelistedEvent {
                    caller: fixture.officer.clone(),
                    user: fixture.target.clone(),
                }
                .into_val(&fixture.env),
            ),
        ]
    );

    // Revoke by the emergency officer: same caller-tracking guarantee.
    fixture
        .client
        .revoke_whitelist(&fixture.emergency, &fixture.target);
    assert_eq!(
        fixture.env.events().all(),
        vec![
            &fixture.env,
            (
                fixture.client.address.clone(),
                ("whitelist_revoked",).into_val(&fixture.env),
                WhitelistRevokedEvent {
                    caller: fixture.emergency.clone(),
                    user: fixture.target.clone(),
                }
                .into_val(&fixture.env),
            ),
        ]
    );
}

#[test]
fn test_rejected_compliance_transitions_emit_no_events() {
    let fixture = ComplianceFixture::new();
    fixture
        .client
        .whitelist_user(&fixture.officer, &fixture.target);
    let outsider = Address::generate(&fixture.env);

    // Unauthorised revoke of an approved address.
    let outcome = fixture.attempt(ComplianceAction::Revoke, TransitionCaller::NoRole);
    assert_eq!(outcome, TransitionOutcome::Rejected(Error::Unauthorized));
    assert_eq!(fixture.last_invocation_event_count(), 0);

    // Wrong-scope role approving a new address.
    let result = fixture
        .client
        .try_whitelist_user(&fixture.manager, &outsider);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
    assert_eq!(fixture.last_invocation_event_count(), 0);

    // Paused (blocked) transitions of both kinds.
    fixture.client.pause(&fixture.admin);
    let outcome = fixture.attempt(
        ComplianceAction::Revoke,
        TransitionCaller::ComplianceOfficer,
    );
    assert_eq!(outcome, TransitionOutcome::Rejected(Error::ContractPaused));
    assert_eq!(fixture.last_invocation_event_count(), 0);

    let result = fixture.client.try_whitelist_user(&fixture.admin, &outsider);
    assert_eq!(result, Err(Ok(Error::ContractPaused)));
    assert_eq!(fixture.last_invocation_event_count(), 0);

    // Soroban also discards events from reverted invocations: neither the
    // approved target nor the untouched outsider changed status.
    assert!(fixture.is_target_approved());
    assert!(!fixture.client.is_whitelisted(&outsider));
}

#[test]
fn test_failed_compliance_transitions_leave_state_consistent() {
    let fixture = ComplianceFixture::new();
    let bystander = Address::generate(&fixture.env);
    let outsider = Address::generate(&fixture.env);

    // Seed realistic state worth protecting.
    fixture
        .client
        .whitelist_user(&fixture.officer, &fixture.target);
    fixture.client.whitelist_user(&fixture.officer, &bystander);
    fixture
        .client
        .mint_asset(&fixture.manager, &fixture.target, &700);
    fixture
        .client
        .mint_asset(&fixture.manager, &bystander, &300);

    // Snapshot every observable a failed transition could plausibly corrupt.
    let target_approved = fixture.is_target_approved();
    let bystander_approved = fixture.client.is_whitelisted(&bystander);
    let outsider_approved = fixture.client.is_whitelisted(&outsider);
    let target_balance = fixture.client.get_balance_of(&fixture.target);
    let bystander_balance = fixture.client.get_balance_of(&bystander);
    let total_supply = fixture.client.get_total_supply();
    let officer_role = fixture.client.get_role_of(&fixture.officer);
    let asset_status = fixture.client.get_asset_status();
    let paused = fixture.client.is_paused();

    // Wave 1: unauthorised callers of both kinds, both actions.
    for caller in [TransitionCaller::NoRole, TransitionCaller::AssetManager] {
        for action in ComplianceAction::ALL {
            let outcome = fixture.attempt(action, caller);
            assert_eq!(outcome, TransitionOutcome::Rejected(Error::Unauthorized));
            assert_eq!(fixture.last_invocation_event_count(), 0);
        }
    }
    let result = fixture
        .client
        .try_whitelist_user(&fixture.intruder, &outsider);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
    let result = fixture
        .client
        .try_revoke_whitelist(&fixture.manager, &outsider);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));

    // Wave 2: the blocked overlay, both actions, authorised callers.
    fixture.client.pause(&fixture.admin);
    let outcome = fixture.attempt(
        ComplianceAction::Revoke,
        TransitionCaller::ComplianceOfficer,
    );
    assert_eq!(outcome, TransitionOutcome::Rejected(Error::ContractPaused));
    let result = fixture
        .client
        .try_whitelist_user(&fixture.emergency, &outsider);
    assert_eq!(result, Err(Ok(Error::ContractPaused)));
    fixture.client.unpause(&fixture.admin);

    // Nothing moved — every rejected transition was atomic, and failures
    // targeting one address never bled into a bystander's state.
    assert_eq!(fixture.is_target_approved(), target_approved);
    assert_eq!(
        fixture.client.is_whitelisted(&bystander),
        bystander_approved
    );
    assert_eq!(fixture.client.is_whitelisted(&outsider), outsider_approved);
    assert_eq!(
        fixture.client.get_balance_of(&fixture.target),
        target_balance
    );
    assert_eq!(fixture.client.get_balance_of(&bystander), bystander_balance);
    assert_eq!(fixture.client.get_total_supply(), total_supply);
    assert_eq!(fixture.client.get_role_of(&fixture.officer), officer_role);
    assert_eq!(fixture.client.get_asset_status(), asset_status);
    assert_eq!(fixture.client.is_paused(), paused);

    // The registry is not wedged by the failures: valid transitions still go
    // through for the same addresses.
    fixture
        .client
        .revoke_whitelist(&fixture.officer, &fixture.target);
    assert!(!fixture.is_target_approved());
    fixture.client.whitelist_user(&fixture.emergency, &outsider);
    assert!(fixture.client.is_whitelisted(&outsider));

    // Revocation freezes the account but never destroys its balance.
    assert_eq!(
        fixture.client.get_balance_of(&fixture.target),
        target_balance
    );



// ─── Holding cap event compatibility (#33) ───────────────────────────────────

#[test]
fn test_holding_cap_proposed_and_amended_emit_events() {
    let (env, client, admin, _user1, _user2) = setup();
    env.mock_all_auths();

    client.initialize(&admin);
    client.propose_holding_cap(&admin, &300);

    // Propose event
    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("holding_cap_proposed",).into_val(&env),
                crate::holding::HoldingCapProposedEvent {
                    admin,
                    current_cap: 0,
                    proposed_cap: 300,
                }
                .into_val(&env),
            ),
        ]
    );

    client.accept_holding_cap(&admin);

    // Amended event
    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                ("holding_cap_amended",).into_val(&env),
                crate::holding::HoldingCapAmendedEvent {
                    admin,
                    previous_cap: 0,
                    new_cap: 300,
                }
                .into_val(&env),
            ),
        ]
    );

}

#[test]
fn test_compliance_status_lifecycle_invariants() {
    // A full state-machine walk: Unknown → Pending → Approved → Revoked →
    // Re-approved, asserting the transition invariants that prevent compliance
    // bypass and lockout bugs at every hop.
    let fixture = ComplianceFixture::new();
    let investor = fixture.target.clone();
    let recipient = Address::generate(&fixture.env);

    // ── Unknown ───────────────────────────────────────────────────────────
    // Never targeted: off the whitelist, and assets cannot be minted to it.
    assert_eq!(
        fixture.is_target_approved(),
        ComplianceStatus::Unknown.is_approved()
    );
    let result = fixture
        .client
        .try_mint_asset(&fixture.manager, &investor, &100);
    assert_eq!(result, Err(Ok(Error::ReceiverNotWhitelisted)));

    // ── Pending ───────────────────────────────────────────────────────────
    // A rejected approval attempt (unauthorised caller) leaves the address
    // off the whitelist — it is pending, and minting stays blocked.
    let outcome = fixture.attempt(ComplianceAction::Approve, TransitionCaller::NoRole);
    assert_eq!(outcome, TransitionOutcome::Rejected(Error::Unauthorized));
    assert!(!fixture.is_target_approved());
    let result = fixture
        .client
        .try_mint_asset(&fixture.manager, &investor, &100);
    assert_eq!(result, Err(Ok(Error::ReceiverNotWhitelisted)));

    // A blocked approval attempt while paused also leaves it pending.
    fixture.client.pause(&fixture.admin);
    let outcome = fixture.attempt(
        ComplianceAction::Approve,
        TransitionCaller::ComplianceOfficer,
    );
    assert_eq!(outcome, TransitionOutcome::Rejected(Error::ContractPaused));
    assert!(!fixture.is_target_approved());
    fixture.client.unpause(&fixture.admin);

    // ── Approved ──────────────────────────────────────────────────────────
    // A committed approval flips the status and unlocks receiving.
    fixture.client.whitelist_user(&fixture.officer, &investor);
    assert!(fixture.is_target_approved());
    fixture.client.mint_asset(&fixture.manager, &investor, &100);
    assert_eq!(fixture.client.get_balance_of(&investor), 100);

    // ── Revoked ───────────────────────────────────────────────────────────
    // Revocation locks sending AND receiving for the investor...
    fixture.client.revoke_whitelist(&fixture.officer, &investor);
    assert!(!fixture.is_target_approved());
    fixture.client.whitelist_user(&fixture.officer, &recipient);

    let result = fixture
        .client
        .try_mint_asset(&fixture.manager, &investor, &50);
    assert_eq!(result, Err(Ok(Error::ReceiverNotWhitelisted)));
    let result = fixture.client.try_transfer(&investor, &recipient, &50);
    assert_eq!(result, Err(Ok(Error::SenderNotWhitelisted)));
    // ...while the balance itself is preserved, not destroyed.
    assert_eq!(fixture.client.get_balance_of(&investor), 100);

    // ── Re-approved ───────────────────────────────────────────────────────
    // Revocation is not a permanent lockout: an authorised caller can
    // re-approve, restoring full movement rights.
    fixture.client.whitelist_user(&fixture.emergency, &investor);
    assert!(fixture.is_target_approved());
    fixture.client.transfer(&investor, &recipient, &40);

    assert_eq!(fixture.client.get_balance_of(&investor), 60);
    assert_eq!(fixture.client.get_balance_of(&recipient), 40);
    assert_eq!(fixture.client.get_total_supply(), 100);
}

#[test]
fn test_compliance_transitions_rejected_after_officer_role_revoked() {
    // Wrong-caller nuance: the moment the admin strips a compliance officer's
    // role, that officer's transitions become invalid — rejected, state
    // unchanged, no event — even for addresses they personally approved.
    let fixture = ComplianceFixture::new();
    fixture
        .client
        .whitelist_user(&fixture.officer, &fixture.target);
    assert!(fixture.is_target_approved());
    let outsider = Address::generate(&fixture.env);

    fixture.client.remove_role(&fixture.admin, &fixture.officer);

    let outcome = fixture.attempt(
        ComplianceAction::Revoke,
        TransitionCaller::ComplianceOfficer,
    );
    assert_eq!(outcome, TransitionOutcome::Rejected(Error::Unauthorized));
    assert!(fixture.is_target_approved());
    assert_eq!(fixture.last_invocation_event_count(), 0);

    let result = fixture
        .client
        .try_whitelist_user(&fixture.officer, &outsider);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
    assert!(!fixture.client.is_whitelisted(&outsider));

    // The admin can still act — removing one officer never wedges the registry.
    fixture
        .client
        .revoke_whitelist(&fixture.admin, &fixture.target);
    assert!(!fixture.is_target_approved());
}
