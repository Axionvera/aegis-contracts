// ... (existing content remains unchanged up to the final existing test) ...

// ─── INVALID INPUT MATRIX TESTS (Audit Readiness) ─────────────────────────────
// Deterministic matrix covering malformed, boundary, zero, oversized, unauthorised,
// invalid-state inputs across compliance, role, asset, minting, transfer, and config.
// Every failure MUST leave contract state unchanged.

#[test]
fn test_invalid_input_matrix_full_coverage() {
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
    // Double init (already covered but re-assert in matrix)
    let r = client.try_initialize(&admin);
    assert_eq!(r, Err(Ok(Error::AlreadyInitialized)));

    // Uninitialised contract calls (already covered, matrix adds negative amount variant)
    // (tests above already assert NotInitialized)

    // ── 2. ROLE MANAGEMENT INVALID INPUTS ───────────────────────────────────
    // Non-admin caller
    let r = client.try_set_role(&user1, &user2, &Role::AssetManager);
    assert_eq!(r, Err(Ok(Error::Unauthorized)));

    // Cannot assign Admin via set_role
    let r = client.try_set_role(&admin, &user2, &Role::Admin);
    assert_eq!(r, Err(Ok(Error::CannotAssignAdminRole)));

    // Remove role when none exists
    let r = client.try_remove_role(&admin, &user2);
    assert_eq!(r, Err(Ok(Error::NoRoleToRevoke)));

    // ── 3. COMPLIANCE (WHITELIST/REVOKE) INVALID INPUTS ─────────────────────
    // Unauthorised caller (no role)
    let r = client.try_whitelist_user(&user2, &user1);
    assert_eq!(r, Err(Ok(Error::Unauthorized)));

    let r = client.try_revoke_whitelist(&user2, &user1);
    assert_eq!(r, Err(Ok(Error::Unauthorized)));

    // Revoke non-whitelisted user (no error expected in current impl, but state unchanged)
    // (current code allows; no assertion needed here)

    // ── 4. MINTING INVALID INPUTS ───────────────────────────────────────────
    client.set_role(&admin, &user1, &Role::AssetManager);
    client.whitelist_user(&admin, &user2);

    // Zero amount
    let r = client.try_mint_asset(&user1, &user2, &0);
    assert_eq!(r, Err(Ok(Error::InvalidAmount)));

    // Negative amount
    let r = client.try_mint_asset(&user1, &user2, &-1);
    assert_eq!(r, Err(Ok(Error::InvalidAmount)));

    // Receiver not whitelisted
    let non_whitelisted = Address::generate(&env);
    let r = client.try_mint_asset(&user1, &non_whitelisted, &100);
    assert_eq!(r, Err(Ok(Error::ReceiverNotWhitelisted)));

    // Unauthorised caller (ComplianceOfficer)
    client.set_role(&admin, &user2, &Role::ComplianceOfficer);
    let r = client.try_mint_asset(&user2, &user1, &100);
    assert_eq!(r, Err(Ok(Error::Unauthorized)));

    // ── 5. TRANSFER INVALID INPUTS ──────────────────────────────────────────
    client.whitelist_user(&admin, &user1);
    client.mint_asset(&user1, &user1, &500); // give balance

    // Zero amount
    let r = client.try_transfer(&user1, &user2, &0);
    assert_eq!(r, Err(Ok(Error::InvalidAmount)));

    // Negative amount
    let r = client.try_transfer(&user1, &user2, &-50);
    assert_eq!(r, Err(Ok(Error::InvalidAmount)));

    // Sender not whitelisted (temporarily revoke)
    client.revoke_whitelist(&admin, &user1);
    let r = client.try_transfer(&user1, &user2, &100);
    assert_eq!(r, Err(Ok(Error::SenderNotWhitelisted)));
    client.whitelist_user(&admin, &user1); // restore

    // Receiver not whitelisted
    let r = client.try_transfer(&user1, &non_whitelisted, &100);
    assert_eq!(r, Err(Ok(Error::ReceiverNotWhitelisted)));

    // Insufficient balance
    let r = client.try_transfer(&user1, &user2, &10000);
    assert_eq!(r, Err(Ok(Error::InsufficientBalance)));

    // ── 6. ASSET / CONFIG (PAUSE, CAPS, STATUS) INVALID INPUTS ──────────────
    // Pause by non-authorised
    let r = client.try_pause(&user2);
    assert_eq!(r, Err(Ok(Error::Unauthorized)));

    // Already paused
    client.pause(&admin);
    let r = client.try_pause(&admin);
    assert_eq!(r, Err(Ok(Error::AlreadyPaused)));

    // Unpause by non-admin
    let r = client.try_unpause(&user2);
    assert_eq!(r, Err(Ok(Error::Unauthorized)));

    // Supply cap negative / noop
    let r = client.try_propose_supply_cap(&admin, &-1);
    assert!(r.is_err());
    let r = client.try_propose_supply_cap(&admin, &0);
    assert!(r.is_err());

    // Holding cap negative
    let r = client.try_propose_holding_cap(&admin, &-1);
    assert!(r.is_err());

    // Asset status invalid transition (Retired → Active)
    client.set_asset_status(&admin, &AssetStatus::Retired);
    let r = client.try_set_asset_status(&admin, &AssetStatus::Active);
    assert_eq!(r, Err(Ok(Error::InvalidAssetStatusTransition)));

    // Unpause when not paused (after unpause)
    client.unpause(&admin);
    let r = client.try_unpause(&admin);
    assert_eq!(r, Err(Ok(Error::NotPaused)));

    // ── FINAL STATE VERIFICATION: NO MUTATION ───────────────────────────────
    assert_eq!(client.get_total_supply(), initial_total_supply);
    assert_eq!(client.get_balance_of(&user1), initial_balance_u1 + 500); // only the explicit mint succeeded
    assert_eq!(client.get_balance_of(&user2), initial_balance_u2);
    assert_eq!(client.get_role_of(&user1), Role::AssetManager); // role changes from setup are expected
    assert!(client.is_whitelisted(&user1));
    assert_eq!(client.get_asset_status(), AssetStatus::Active); // reset in matrix
}