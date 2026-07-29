//! Event compatibility tests for SDK, dashboard, and indexer consumers.
//!
//! These tests intentionally overlap the SDK fixture generator. The fixture
//! test proves the committed JSON examples stay byte-for-byte stable; this file
//! keeps a compact, human-readable compatibility matrix in the normal
//! integration-test suite so event topic or payload-shape drift is visible
//! without reading the fixture generator.

mod support;

use soroban_sdk::{IntoVal, String as SorobanString};

use aegis_contracts::admin::{
    AdminTransferInitiatedEvent, AdminTransferredEvent, ContractPausedEvent, ContractUnpausedEvent,
    RoleAssignedEvent, RoleRevokedEvent,
};
use aegis_contracts::asset::{
    AssetMetadataUpdatedEvent, AssetMintedEvent, TransferEvent, YieldDistributedEvent,
};
use aegis_contracts::compliance::{
    ComplianceStatus, ComplianceStatusChangedEvent, UserWhitelistedEvent, WhitelistRevokedEvent,
};
use aegis_contracts::config::{ConfigAmendedEvent, ConfigProposedEvent, ProtocolConfig};
use aegis_contracts::holding::{HoldingCapAmendedEvent, HoldingCapProposedEvent};
use aegis_contracts::issuer::{IssuerSeparationPolicy, IssuerSeparationPolicyUpdatedEvent};
use aegis_contracts::lifecycle::{AssetStatus, AssetStatusChangedEvent};
use aegis_contracts::supply_cap::{SupplyCapAmendedEvent, SupplyCapProposedEvent};
use aegis_contracts::{ContractInitializedEvent, Error, Role};

use support::Harness;

macro_rules! assert_compat_events {
    ($harness:expr, $(($topic:literal, $payload:expr)),+ $(,)?) => {{
        let h = &$harness;
        h.assert_events(soroban_sdk::vec![
            &h.env,
            $((
                h.contract_id.clone(),
                ($topic,).into_val(&h.env),
                $payload.into_val(&h.env),
            )),+
        ]);
    }};
}

fn bootstrap() -> Harness {
    let h = Harness::new();
    let c = h.client();

    c.initialize(&h.actor("admin"));
    c.set_role(
        &h.actor("admin"),
        &h.actor("compliance_officer"),
        &Role::ComplianceOfficer,
    );
    c.set_role(
        &h.actor("admin"),
        &h.actor("asset_manager"),
        &Role::AssetManager,
    );
    c.set_role(
        &h.actor("admin"),
        &h.actor("emergency_officer"),
        &Role::EmergencyOfficer,
    );
    c.set_asset_status(&h.actor("admin"), &AssetStatus::Active);
    c.whitelist_user(&h.actor("compliance_officer"), &h.actor("investor_alice"));
    c.whitelist_user(&h.actor("compliance_officer"), &h.actor("investor_bob"));

    h
}

#[test]
fn compliance_events_keep_sdk_dashboard_shape() {
    let h = bootstrap();
    let c = h.client();

    c.whitelist_user(&h.actor("compliance_officer"), &h.actor("investor_carol"));
    assert_compat_events!(
        h,
        (
            "compliance_status_changed",
            ComplianceStatusChangedEvent {
                caller: h.actor("compliance_officer"),
                user: h.actor("investor_carol"),
                previous_status: ComplianceStatus::Unknown,
                new_status: ComplianceStatus::Approved,
            }
        ),
        (
            "user_whitelisted",
            UserWhitelistedEvent {
                caller: h.actor("compliance_officer"),
                user: h.actor("investor_carol"),
            }
        ),
    );

    c.revoke_whitelist(&h.actor("compliance_officer"), &h.actor("investor_carol"));
    assert_compat_events!(
        h,
        (
            "compliance_status_changed",
            ComplianceStatusChangedEvent {
                caller: h.actor("compliance_officer"),
                user: h.actor("investor_carol"),
                previous_status: ComplianceStatus::Approved,
                new_status: ComplianceStatus::Revoked,
            }
        ),
        (
            "whitelist_revoked",
            WhitelistRevokedEvent {
                caller: h.actor("compliance_officer"),
                user: h.actor("investor_carol"),
            }
        ),
    );
}

#[test]
fn minting_transfer_and_reverted_transfer_events_keep_shape() {
    let h = bootstrap();
    let c = h.client();

    c.mint_asset(
        &h.actor("asset_manager"),
        &h.actor("investor_alice"),
        &1_000,
    );
    assert_compat_events!(
        h,
        (
            "asset_minted",
            AssetMintedEvent {
                caller: h.actor("asset_manager"),
                to: h.actor("investor_alice"),
                amount: 1_000,
                total_supply: 1_000,
            }
        ),
    );

    c.transfer(&h.actor("investor_alice"), &h.actor("investor_bob"), &250);
    assert_compat_events!(
        h,
        (
            "transfer",
            TransferEvent {
                from: h.actor("investor_alice"),
                to: h.actor("investor_bob"),
                amount: 250,
            }
        ),
    );

    c.distribute_yield(&h.actor("asset_manager"), &500);
    assert_compat_events!(
        h,
        (
            "yield_distributed",
            YieldDistributedEvent {
                caller: h.actor("asset_manager"),
                amount: 500,
            }
        ),
    );

    let result = c.try_transfer(&h.actor("investor_alice"), &h.actor("outsider_dave"), &100);
    assert_eq!(result, Err(Ok(Error::ReceiverNotWhitelisted)));
    h.assert_no_events();
}

#[test]
fn role_admin_pause_and_governance_events_keep_shape() {
    let h = Harness::new();
    let c = h.client();

    c.initialize(&h.actor("admin"));
    assert_compat_events!(
        h,
        (
            "contract_initialized",
            ContractInitializedEvent {
                admin: h.actor("admin"),
            }
        ),
    );

    c.set_role(
        &h.actor("admin"),
        &h.actor("compliance_officer"),
        &Role::ComplianceOfficer,
    );
    assert_compat_events!(
        h,
        (
            "role_assigned",
            RoleAssignedEvent {
                admin: h.actor("admin"),
                target: h.actor("compliance_officer"),
                role: Role::ComplianceOfficer,
            }
        ),
    );

    c.remove_role(&h.actor("admin"), &h.actor("compliance_officer"));
    assert_compat_events!(
        h,
        (
            "role_revoked",
            RoleRevokedEvent {
                admin: h.actor("admin"),
                target: h.actor("compliance_officer"),
                role: Role::ComplianceOfficer,
            }
        ),
    );

    c.transfer_admin(&h.actor("admin"), &h.actor("investor_carol"));
    assert_compat_events!(
        h,
        (
            "admin_transfer_initiated",
            AdminTransferInitiatedEvent {
                current_admin: h.actor("admin"),
                candidate: h.actor("investor_carol"),
            }
        ),
    );

    c.accept_admin(&h.actor("investor_carol"));
    assert_compat_events!(
        h,
        (
            "admin_transferred",
            AdminTransferredEvent {
                previous_admin: h.actor("admin"),
                new_admin: h.actor("investor_carol"),
            }
        ),
    );

    c.renounce_admin(&h.actor("investor_carol"));
    assert_compat_events!(
        h,
        (
            "admin_renounced",
            AdminTransferredEvent {
                previous_admin: h.actor("investor_carol"),
                new_admin: h.actor("investor_carol"),
            }
        ),
    );

    let h = bootstrap();
    let c = h.client();
    c.pause(&h.actor("emergency_officer"));
    assert_compat_events!(
        h,
        (
            "contract_paused",
            ContractPausedEvent {
                admin: h.actor("emergency_officer"),
            }
        ),
    );

    c.unpause(&h.actor("admin"));
    assert_compat_events!(
        h,
        (
            "contract_unpaused",
            ContractUnpausedEvent {
                admin: h.actor("admin"),
            }
        ),
    );
}

#[test]
fn asset_lifecycle_metadata_and_configuration_events_keep_shape() {
    let h = bootstrap();
    let c = h.client();

    c.set_asset_status(&h.actor("admin"), &AssetStatus::Paused);
    assert_compat_events!(
        h,
        (
            "asset_status_changed",
            AssetStatusChangedEvent {
                admin: h.actor("admin"),
                previous_status: AssetStatus::Active,
                new_status: AssetStatus::Paused,
            }
        ),
    );

    c.set_asset_status(&h.actor("admin"), &AssetStatus::Active);
    let name = SorobanString::from_str(&h.env, "Aegis Sample Tower");
    let symbol = SorobanString::from_str(&h.env, "AST");
    let uri = SorobanString::from_str(&h.env, "https://example.invalid/aegis/sample-tower.json");
    c.update_asset_metadata(&h.actor("asset_manager"), &name, &symbol, &uri);
    assert_compat_events!(
        h,
        (
            "asset_metadata_updated",
            AssetMetadataUpdatedEvent {
                caller: h.actor("asset_manager"),
                name,
                symbol,
                uri,
            }
        ),
    );

    c.propose_supply_cap(&h.actor("admin"), &10_000);
    assert_compat_events!(
        h,
        (
            "supply_cap_proposed",
            SupplyCapProposedEvent {
                admin: h.actor("admin"),
                current_cap: 0,
                proposed_cap: 10_000,
            }
        ),
    );

    c.accept_supply_cap(&h.actor("admin"));
    assert_compat_events!(
        h,
        (
            "supply_cap_amended",
            SupplyCapAmendedEvent {
                admin: h.actor("admin"),
                previous_cap: 0,
                new_cap: 10_000,
            }
        ),
    );

    c.propose_holding_cap(&h.actor("admin"), &2_000);
    assert_compat_events!(
        h,
        (
            "holding_cap_proposed",
            HoldingCapProposedEvent {
                admin: h.actor("admin"),
                current_cap: 0,
                proposed_cap: 2_000,
            }
        ),
    );

    c.accept_holding_cap(&h.actor("admin"));
    assert_compat_events!(
        h,
        (
            "holding_cap_amended",
            HoldingCapAmendedEvent {
                admin: h.actor("admin"),
                previous_cap: 0,
                new_cap: 2_000,
            }
        ),
    );

    let config = ProtocolConfig {
        min_transfer_amount: 100,
        max_batch_size: 50,
    };
    c.propose_config(&h.actor("admin"), &config);
    assert_compat_events!(
        h,
        (
            "config_proposed",
            ConfigProposedEvent {
                admin: h.actor("admin"),
                proposed_config: config.clone(),
            }
        ),
    );

    c.accept_config(&h.actor("admin"));
    assert_compat_events!(
        h,
        (
            "config_amended",
            ConfigAmendedEvent {
                admin: h.actor("admin"),
                new_config: config,
            }
        ),
    );

    let policy = IssuerSeparationPolicy {
        enforced: true,
        allow_dual_duty_issuance: false,
        allow_self_issuance: false,
        require_independent_approver: true,
    };
    c.set_issuer_separation_policy(&h.actor("admin"), &policy);
    assert_compat_events!(
        h,
        (
            "issuer_separation_policy_updated",
            IssuerSeparationPolicyUpdatedEvent {
                admin: h.actor("admin"),
                previous_policy: IssuerSeparationPolicy::default_policy(),
                new_policy: policy,
            }
        ),
    );
}
