//! Deterministic SDK integration fixtures for the Aegis RWA contracts.
//!
//! # What this is
//!
//! Downstream repositories (the TypeScript/Rust SDK, the dashboard, indexers)
//! need stable, known-good examples of what this contract *actually* returns:
//! the shape of a compliance read, the exact topic and payload of an
//! `asset_minted` event, the numeric code behind a rejected transfer. Without
//! a shared source of truth each repo hand-writes its own mock, those mocks
//! drift from the contract, and the drift is only discovered on testnet.
//!
//! This harness closes that gap. Every scenario below drives the **real
//! contract** through the Soroban test host and serialises the observed
//! results — return values, emitted events, and error codes — to JSON under
//! [`fixtures/sdk/`](../fixtures/sdk). Nothing is hand-written: values are
//! rendered from the wire-level `ScVal`/XDR an SDK would receive.
//!
//! # Two jobs, one harness
//!
//! 1. **Publish** fixtures for cross-repo consumption
//!    (`UPDATE_FIXTURES=1 cargo test --test sdk_fixtures` rewrites them).
//! 2. **Guard** them: on a normal `cargo test` run each scenario is
//!    regenerated and compared byte-for-byte against the committed file, so
//!    contract drift fails CI here rather than silently shipping a stale
//!    fixture to the SDK.
//!
//! See `docs/sdk-fixtures.md` for the consumer-facing contract.

mod support;

use soroban_sdk::{Address, String as SorobanString};

use aegis_contracts::asset::AssetStatus;
use aegis_contracts::{Error, Role};

use support::{
    assert_unique_ids, envelope, write_or_verify, Harness, Json, JsonObj, Scenario, ACTORS,
    ADDRESS_DERIVATION_SEED, CONTRACT_ADDRESS,
};

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Renders the result of a `try_*` client call as a fixture value.
///
/// Contract-defined failures (`Result::Err` returns from the contract) are
/// rendered as `{"ok": false, "error": {...}}` carrying the stable numeric
/// code from `docs/error-codes.md`. Host panics (from `assert!`-style
/// invariants such as the supply/holding caps) surface as a host trap with no
/// contract code, and are rendered as `{"ok": false, "error": {"type": "host"}}`.
fn ok_result() -> Json {
    let mut o = JsonObj::new();
    o.push("ok", Json::Bool(true));
    o.build()
}

fn contract_error(code: u32, name: &str, category: &str) -> Json {
    let mut err = JsonObj::new();
    err.push("type", Json::str("contract"));
    err.push("code", Json::Num(code as i128));
    err.push("name", Json::str(name));
    err.push("category", Json::str(category));

    let mut o = JsonObj::new();
    o.push("ok", Json::Bool(false));
    o.push("error", err.build());
    o.build()
}

fn host_error(reason: &str) -> Json {
    let mut err = JsonObj::new();
    err.push("type", Json::str("host"));
    err.push("code", Json::Null);
    err.push("reason", Json::str(reason));
    err.push(
        "sdk_guidance",
        Json::str(
            "Host traps carry no contract error code. SDKs must fail safe and \
             surface a generic failure rather than assuming a numeric code.",
        ),
    );

    let mut o = JsonObj::new();
    o.push("ok", Json::Bool(false));
    o.push("error", err.build());
    o.build()
}

/// Maps an `Error` variant to its documented category (see `docs/error-codes.md`).
fn category_of(code: u32) -> &'static str {
    match code / 1000 {
        1 => "configuration",
        2 => "storage",
        3 => "admin_authorization",
        4 => "compliance",
        5 => "minting_transfers",
        6 => "asset_metadata",
        _ => "unknown",
    }
}

fn err_json(e: Error) -> Json {
    let code = e as u32;
    contract_error(code, error_name(e), category_of(code))
}

fn error_name(e: Error) -> &'static str {
    match e {
        Error::AlreadyInitialized => "AlreadyInitialized",
        Error::NotInitialized => "NotInitialized",
        Error::NoPendingAdminTransfer => "NoPendingAdminTransfer",
        Error::Unauthorized => "Unauthorized",
        Error::CannotAssignAdminRole => "CannotAssignAdminRole",
        Error::NoRoleToRevoke => "NoRoleToRevoke",
        Error::NotPendingCandidate => "NotPendingCandidate",
        Error::ContractPaused => "ContractPaused",
        Error::AlreadyPaused => "AlreadyPaused",
        Error::NotPaused => "NotPaused",
        Error::SenderNotWhitelisted => "SenderNotWhitelisted",
        Error::ReceiverNotWhitelisted => "ReceiverNotWhitelisted",
        Error::InvalidAmount => "InvalidAmount",
        Error::InsufficientBalance => "InsufficientBalance",
        Error::AssetNotActive => "AssetNotActive",
        Error::InvalidAssetStatusTransition => "InvalidAssetStatusTransition",
        Error::AssetMetadataUpdateBlocked => "AssetMetadataUpdateBlocked",
    }
}

/// Renders a `try_*` outcome, asserting it matches the expected variant so the
/// fixture and the assertion can never disagree.
fn expect_err<T: core::fmt::Debug>(
    result: Result<T, Result<Error, soroban_sdk::InvokeError>>,
    expected: Error,
) -> Json {
    match result {
        Err(Ok(actual)) => {
            assert_eq!(actual, expected, "unexpected contract error variant");
            err_json(actual)
        }
        other => panic!("expected contract error {expected:?}, got {other:?}"),
    }
}

/// Bootstraps the standard fixture world: initialised contract, roles granted,
/// investors whitelisted.
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
    c.whitelist_user(&h.actor("compliance_officer"), &h.actor("investor_alice"));
    c.whitelist_user(&h.actor("compliance_officer"), &h.actor("investor_bob"));
    h
}

// ─── 00 · Actors ──────────────────────────────────────────────────────────────

/// Publishes the synthetic identity table so SDK repos can pin the exact same
/// addresses instead of inventing their own.
#[test]
fn fixture_actors() {
    let h = Harness::new();

    let mut actors = Vec::new();
    for (label, key) in ACTORS {
        // Round-trip every strkey through the host so a malformed constant
        // fails here rather than in a downstream repo.
        let addr = Address::from_str(&h.env, key);
        assert_eq!(addr.to_string().to_string(), *key);

        let mut o = JsonObj::new();
        o.push("label", Json::str(*label));
        o.push("address", Json::str(*key));
        o.push("kind", Json::str("account"));
        actors.push(o.build());
    }

    let mut contract = JsonObj::new();
    contract.push("label", Json::str("contract"));
    contract.push("address", Json::str(CONTRACT_ADDRESS));
    contract.push("kind", Json::str("contract"));
    actors.push(contract.build());

    let scenario = Scenario::new(
        "synthetic-identities",
        "Fixed, synthetic addresses used by every fixture in this directory.",
    )
    .set("derivation", Json::str(ADDRESS_DERIVATION_SEED))
    .set(
        "derivation_detail",
        Json::str(
            "Ed25519 public key bytes = SHA-256 of the UTF-8 seed string; \
             encoded as a Stellar strkey (G… for accounts, C… for the contract).",
        ),
    )
    .set(
        "privacy",
        Json::str(
            "No private keys exist for these addresses, they are unfunded on all \
             networks, and they correspond to no real person or account.",
        ),
    )
    .set("actors", Json::Arr(actors))
    .build();

    let scenarios = vec![scenario];
    assert_unique_ids(&scenarios);
    write_or_verify(
        "00-actors.json",
        &envelope(
            "00-actors",
            "Synthetic identity table shared by all Aegis SDK fixtures.",
            scenarios,
        ),
    );
}

// ─── 01 · Compliance reads ────────────────────────────────────────────────────

/// Compliance read surface: `is_whitelisted` before/after whitelisting and
/// after revocation, plus the events each compliance write emits.
#[test]
fn fixture_compliance() {
    let mut scenarios = Vec::new();

    // 1 — a brand-new address is not whitelisted.
    {
        let h = Harness::new();
        let c = h.client();
        c.initialize(&h.actor("admin"));

        let dave = h.actor("outsider_dave");
        let whitelisted = c.is_whitelisted(&dave);
        assert!(!whitelisted);

        scenarios.push(
            Scenario::new(
                "is-whitelisted-unknown-address",
                "An address that was never whitelisted reads false. This is the \
                 default state for every address, including the admin.",
            )
            .set("call", Json::str("is_whitelisted"))
            .set("args", Json::Arr(vec![Json::str("outsider_dave")]))
            .set("returns", h.render(whitelisted))
            .build(),
        );
    }

    // 2 — whitelisting flips the read and emits `user_whitelisted`.
    {
        let h = Harness::new();
        let c = h.client();
        c.initialize(&h.actor("admin"));
        c.set_role(
            &h.actor("admin"),
            &h.actor("compliance_officer"),
            &Role::ComplianceOfficer,
        );

        let officer = h.actor("compliance_officer");
        let alice = h.actor("investor_alice");
        c.whitelist_user(&officer, &alice);
        let events = h.events();
        let whitelisted = c.is_whitelisted(&alice);
        assert!(whitelisted);

        scenarios.push(
            Scenario::new(
                "whitelist-user-success",
                "A ComplianceOfficer whitelists an investor. The read flips to true \
                 and a `user_whitelisted` event is emitted.",
            )
            .set("call", Json::str("whitelist_user"))
            .set(
                "args",
                Json::Arr(vec![
                    Json::str("compliance_officer"),
                    Json::str("investor_alice"),
                ]),
            )
            .set("result", ok_result())
            .set("events", events)
            .set("is_whitelisted_after", h.render(whitelisted))
            .build(),
        );
    }

    // 3 — revocation flips it back and emits `whitelist_revoked`.
    {
        let h = Harness::new();
        let c = h.client();
        c.initialize(&h.actor("admin"));
        c.set_role(
            &h.actor("admin"),
            &h.actor("compliance_officer"),
            &Role::ComplianceOfficer,
        );
        let officer = h.actor("compliance_officer");
        let alice = h.actor("investor_alice");
        c.whitelist_user(&officer, &alice);

        c.revoke_whitelist(&officer, &alice);
        let events = h.events();
        let whitelisted = c.is_whitelisted(&alice);
        assert!(!whitelisted);

        scenarios.push(
            Scenario::new(
                "revoke-whitelist-success",
                "Revoking compliance approval flips the read back to false and emits \
                 a `whitelist_revoked` event.",
            )
            .set("call", Json::str("revoke_whitelist"))
            .set(
                "args",
                Json::Arr(vec![
                    Json::str("compliance_officer"),
                    Json::str("investor_alice"),
                ]),
            )
            .set("result", ok_result())
            .set("events", events)
            .set("is_whitelisted_after", h.render(whitelisted))
            .build(),
        );
    }

    // 4 — role read surface.
    {
        let h = bootstrap();
        let c = h.client();

        let mut roles = JsonObj::new();
        for label in [
            "admin",
            "compliance_officer",
            "asset_manager",
            "emergency_officer",
            "outsider_dave",
        ] {
            roles.push_owned(label.to_string(), h.render(c.get_role_of(&h.actor(label))));
        }

        scenarios.push(
            Scenario::new(
                "get-role-of-all-actors",
                "Role reads for every actor. A `#[contracttype]` unit enum is encoded \
                 on the wire as a single-element vector holding the variant name, so \
                 `Role::Admin` renders as [\"Admin\"] and an unassigned address as [\"None\"].",
            )
            .set("call", Json::str("get_role_of"))
            .set("returns_by_actor", roles.build())
            .build(),
        );
    }

    assert_unique_ids(&scenarios);
    write_or_verify(
        "01-compliance.json",
        &envelope(
            "01-compliance",
            "Compliance whitelist reads, writes, and the events they emit.",
            scenarios,
        ),
    );
}

// ─── 02 · Minting ─────────────────────────────────────────────────────────────

/// Minting surface: successful mints, running total supply, and the balance
/// reads an SDK uses to confirm them.
#[test]
fn fixture_minting() {
    let mut scenarios = Vec::new();

    // 1 — first mint to a whitelisted investor.
    {
        let h = bootstrap();
        let c = h.client();
        let manager = h.actor("asset_manager");
        let alice = h.actor("investor_alice");

        let supply_before = c.get_total_supply();
        let balance_before = c.get_balance_of(&alice);
        c.mint_asset(&manager, &alice, &1_000);
        let events = h.events();

        assert_eq!(c.get_balance_of(&alice), 1_000);
        assert_eq!(c.get_total_supply(), 1_000);

        scenarios.push(
            Scenario::new(
                "mint-first-issuance",
                "An AssetManager mints 1000 units to a whitelisted investor. This is \
                 the canonical issuance event; for a recipient's first mint it also \
                 marks their effective registration as a holder (see docs/events.md).",
            )
            .set("call", Json::str("mint_asset"))
            .set(
                "args",
                Json::Arr(vec![
                    Json::str("asset_manager"),
                    Json::str("investor_alice"),
                    Json::str("1000"),
                ]),
            )
            .set("total_supply_before", h.render(supply_before))
            .set("balance_before", h.render(balance_before))
            .set("result", ok_result())
            .set("events", events)
            .set("balance_after", h.render(c.get_balance_of(&alice)))
            .set("total_supply_after", h.render(c.get_total_supply()))
            .build(),
        );
    }

    // 2 — second mint accumulates supply.
    {
        let h = bootstrap();
        let c = h.client();
        let manager = h.actor("asset_manager");
        let alice = h.actor("investor_alice");
        let bob = h.actor("investor_bob");

        c.mint_asset(&manager, &alice, &400);
        c.mint_asset(&manager, &bob, &600);
        let events = h.events();

        assert_eq!(c.get_total_supply(), 1_000);

        let mut balances = JsonObj::new();
        balances.push("investor_alice", h.render(c.get_balance_of(&alice)));
        balances.push("investor_bob", h.render(c.get_balance_of(&bob)));

        scenarios.push(
            Scenario::new(
                "mint-running-total-supply",
                "A second mint to a different holder. The `asset_minted` event carries \
                 the cumulative `total_supply` (1000), not the amount just minted (600) \
                 — the single most common SDK misreading of this event.",
            )
            .set("call", Json::str("mint_asset"))
            .set(
                "sequence",
                Json::Arr(vec![
                    Json::str("mint_asset(asset_manager, investor_alice, 400)"),
                    Json::str("mint_asset(asset_manager, investor_bob, 600)"),
                ]),
            )
            .set("result", ok_result())
            .set(
                "events",
                Json::Obj(vec![
                    (
                        "note".into(),
                        Json::str(
                            "Soroban's test host exposes only the most recent invocation's \
                         events; this array is from the second mint.",
                        ),
                    ),
                    ("emitted".into(), events),
                ]),
            )
            .set("balances_after", balances.build())
            .set("total_supply_after", h.render(c.get_total_supply()))
            .build(),
        );
    }

    // 3 — admin may mint without an explicit AssetManager grant.
    {
        let h = bootstrap();
        let c = h.client();
        let admin = h.actor("admin");
        let alice = h.actor("investor_alice");

        c.mint_asset(&admin, &alice, &250);
        let events = h.events();

        scenarios.push(
            Scenario::new(
                "mint-by-admin-without-explicit-role",
                "The supreme admin bypasses role checks and can mint directly. Note the \
                 event's `caller` is the admin address.",
            )
            .set("call", Json::str("mint_asset"))
            .set(
                "args",
                Json::Arr(vec![
                    Json::str("admin"),
                    Json::str("investor_alice"),
                    Json::str("250"),
                ]),
            )
            .set("result", ok_result())
            .set("events", events)
            .set("balance_after", h.render(c.get_balance_of(&alice)))
            .build(),
        );
    }

    // 4 — supply cap governance then a cap-bounded mint.
    {
        let h = bootstrap();
        let c = h.client();
        let admin = h.actor("admin");
        let manager = h.actor("asset_manager");
        let alice = h.actor("investor_alice");

        let cap_default = c.get_supply_cap();
        c.propose_supply_cap(&admin, &5_000);
        let proposed_events = h.events();
        let pending = c.get_pending_supply_cap();
        c.accept_supply_cap(&admin);
        let accepted_events = h.events();

        c.mint_asset(&manager, &alice, &5_000);
        assert_eq!(c.get_total_supply(), 5_000);

        scenarios.push(
            Scenario::new(
                "supply-cap-two-step-governance",
                "The 2-step supply cap flow: propose, read the pending value, accept. \
                 A cap of 0 means unbounded. Minting exactly up to the cap succeeds.",
            )
            .set("supply_cap_default", h.render(cap_default))
            .set("propose_events", proposed_events)
            .set("pending_supply_cap", h.render(pending))
            .set("accept_events", accepted_events)
            .set("supply_cap_active", h.render(c.get_supply_cap()))
            .set("pending_after_accept", h.render(c.get_pending_supply_cap()))
            .set("mint_to_cap", ok_result())
            .set("total_supply_after", h.render(c.get_total_supply()))
            .build(),
        );
    }

    // 5 — holding cap read surface.
    {
        let h = bootstrap();
        let c = h.client();
        let admin = h.actor("admin");

        let default_cap = c.get_holding_cap();
        c.propose_holding_cap(&admin, &2_500);
        let events = h.events();
        let pending = c.get_pending_holding_cap();
        c.accept_holding_cap(&admin);
        let accept_events = h.events();

        scenarios.push(
            Scenario::new(
                "holding-cap-two-step-governance",
                "The per-investor holding cap uses the same 2-step propose/accept flow. \
                 0 means unrestricted.",
            )
            .set("holding_cap_default", h.render(default_cap))
            .set("propose_events", events)
            .set("pending_holding_cap", h.render(pending))
            .set("accept_events", accept_events)
            .set("holding_cap_active", h.render(c.get_holding_cap()))
            .build(),
        );
    }

    // 6 — cancelling an outstanding proposal, for both cap types.
    {
        let h = bootstrap();
        let c = h.client();
        let admin = h.actor("admin");

        // Supply cap: propose, then cancel instead of accepting.
        c.propose_supply_cap(&admin, &9_000);
        let pending_supply_before = c.get_pending_supply_cap();
        c.cancel_supply_cap_proposal(&admin);
        let supply_cancel_events = h.events();

        // Holding cap: same flow.
        c.propose_holding_cap(&admin, &3_000);
        let pending_holding_before = c.get_pending_holding_cap();
        c.cancel_holding_cap_proposal(&admin);
        let holding_cancel_events = h.events();

        // Cancelling clears the proposal and leaves the active cap untouched.
        assert_eq!(c.get_pending_supply_cap(), None);
        assert_eq!(c.get_pending_holding_cap(), None);
        assert_eq!(c.get_supply_cap(), 0);
        assert_eq!(c.get_holding_cap(), 0);

        scenarios.push(
            Scenario::new(
                "cap-proposal-cancelled",
                "Cancelling an outstanding cap proposal. Both cancel functions clear \
                 the pending slot and leave the active cap unchanged. Note they emit \
                 NO event — unlike propose/accept — so an SDK tracking governance \
                 state must re-read `get_pending_*` after a cancel rather than \
                 waiting for an event that never arrives.",
            )
            .set(
                "pending_supply_before_cancel",
                h.render(pending_supply_before),
            )
            .set("supply_cancel_events", supply_cancel_events)
            .set(
                "pending_supply_after_cancel",
                h.render(c.get_pending_supply_cap()),
            )
            .set("supply_cap_still_active", h.render(c.get_supply_cap()))
            .set(
                "pending_holding_before_cancel",
                h.render(pending_holding_before),
            )
            .set("holding_cancel_events", holding_cancel_events)
            .set(
                "pending_holding_after_cancel",
                h.render(c.get_pending_holding_cap()),
            )
            .set("holding_cap_still_active", h.render(c.get_holding_cap()))
            .build(),
        );
    }

    assert_unique_ids(&scenarios);
    write_or_verify(
        "02-minting.json",
        &envelope(
            "02-minting",
            "Minting flows, running supply accounting, and supply/holding cap governance.",
            scenarios,
        ),
    );
}

// ─── 03 · Transfers ───────────────────────────────────────────────────────────

/// Transfer surface plus the read-only eligibility helpers an SDK should call
/// before submitting a transfer.
#[test]
fn fixture_transfers() {
    let mut scenarios = Vec::new();

    // 1 — plain successful transfer.
    {
        let h = bootstrap();
        let c = h.client();
        let manager = h.actor("asset_manager");
        let alice = h.actor("investor_alice");
        let bob = h.actor("investor_bob");
        c.mint_asset(&manager, &alice, &1_000);

        let mut before = JsonObj::new();
        before.push("investor_alice", h.render(c.get_balance_of(&alice)));
        before.push("investor_bob", h.render(c.get_balance_of(&bob)));

        c.transfer(&alice, &bob, &250);
        let events = h.events();

        assert_eq!(c.get_balance_of(&alice), 750);
        assert_eq!(c.get_balance_of(&bob), 250);
        assert_eq!(c.get_total_supply(), 1_000);

        let mut after = JsonObj::new();
        after.push("investor_alice", h.render(c.get_balance_of(&alice)));
        after.push("investor_bob", h.render(c.get_balance_of(&bob)));

        scenarios.push(
            Scenario::new(
                "transfer-success-between-whitelisted",
                "A compliant transfer between two whitelisted investors. Total supply \
                 is unchanged — a transfer moves value, it never mints.",
            )
            .set("call", Json::str("transfer"))
            .set(
                "args",
                Json::Arr(vec![
                    Json::str("investor_alice"),
                    Json::str("investor_bob"),
                    Json::str("250"),
                ]),
            )
            .set("balances_before", before.build())
            .set("result", ok_result())
            .set("events", events)
            .set("balances_after", after.build())
            .set("total_supply_after", h.render(c.get_total_supply()))
            .build(),
        );
    }

    // 2 — full-balance transfer drains the sender.
    {
        let h = bootstrap();
        let c = h.client();
        let manager = h.actor("asset_manager");
        let alice = h.actor("investor_alice");
        let bob = h.actor("investor_bob");
        c.mint_asset(&manager, &alice, &500);

        c.transfer(&alice, &bob, &500);
        let events = h.events();
        assert_eq!(c.get_balance_of(&alice), 0);

        scenarios.push(
            Scenario::new(
                "transfer-entire-balance",
                "Transferring the full balance is allowed and leaves the sender at 0. \
                 A zeroed balance is a normal state, not an error.",
            )
            .set("call", Json::str("transfer"))
            .set(
                "args",
                Json::Arr(vec![
                    Json::str("investor_alice"),
                    Json::str("investor_bob"),
                    Json::str("500"),
                ]),
            )
            .set("result", ok_result())
            .set("events", events)
            .set("sender_balance_after", h.render(c.get_balance_of(&alice)))
            .set("receiver_balance_after", h.render(c.get_balance_of(&bob)))
            .build(),
        );
    }

    // 3 — eligibility snapshot for an eligible holder.
    {
        let h = bootstrap();
        let c = h.client();
        let manager = h.actor("asset_manager");
        let alice = h.actor("investor_alice");
        c.mint_asset(&manager, &alice, &1_000);

        let elig = c.get_investor_eligibility(&alice);
        assert!(elig.can_send && elig.can_receive);

        scenarios.push(
            Scenario::new(
                "eligibility-whitelisted-holder",
                "The aggregated eligibility read for a compliant holder with a balance. \
                 `remaining_capacity` is null when no holding cap is active.",
            )
            .set("call", Json::str("get_investor_eligibility"))
            .set("args", Json::Arr(vec![Json::str("investor_alice")]))
            .set("returns", h.render(elig))
            .build(),
        );
    }

    // 4 — eligibility for a non-whitelisted outsider.
    {
        let h = bootstrap();
        let c = h.client();
        let dave = h.actor("outsider_dave");

        let elig = c.get_investor_eligibility(&dave);
        assert!(!elig.can_send && !elig.can_receive);

        scenarios.push(
            Scenario::new(
                "eligibility-non-whitelisted",
                "A non-whitelisted address: both `can_send` and `can_receive` are false. \
                 SDKs should route this user into the KYC flow rather than letting them \
                 build a transaction that will revert with 4000/4001.",
            )
            .set("call", Json::str("get_investor_eligibility"))
            .set("args", Json::Arr(vec![Json::str("outsider_dave")]))
            .set("returns", h.render(elig))
            .build(),
        );
    }

    // 5 — eligibility under an active holding cap.
    {
        let h = bootstrap();
        let c = h.client();
        let admin = h.actor("admin");
        let manager = h.actor("asset_manager");
        let alice = h.actor("investor_alice");

        c.propose_holding_cap(&admin, &1_000);
        c.accept_holding_cap(&admin);
        c.mint_asset(&manager, &alice, &600);

        let elig = c.get_investor_eligibility(&alice);
        assert_eq!(elig.remaining_capacity, Some(400));

        scenarios.push(
            Scenario::new(
                "eligibility-with-holding-cap-headroom",
                "With a 1000 holding cap and a 600 balance, `remaining_capacity` is 400. \
                 SDKs should use this to cap the amount field in a deposit form.",
            )
            .set("holding_cap", h.render(c.get_holding_cap()))
            .set("call", Json::str("get_investor_eligibility"))
            .set("args", Json::Arr(vec![Json::str("investor_alice")]))
            .set("returns", h.render(elig))
            .build(),
        );
    }

    // 6 — check_transfer_eligibility across amounts.
    {
        let h = bootstrap();
        let c = h.client();
        let manager = h.actor("asset_manager");
        let alice = h.actor("investor_alice");
        let bob = h.actor("investor_bob");
        let dave = h.actor("outsider_dave");
        c.mint_asset(&manager, &alice, &1_000);

        let mut cases = Vec::new();
        for (desc, from, to, amount, expected) in [
            ("within balance", &alice, &bob, 250i128, true),
            ("exactly the full balance", &alice, &bob, 1_000, true),
            ("above balance", &alice, &bob, 1_001, false),
            ("zero amount is rejected", &alice, &bob, 0, false),
            ("negative amount is rejected", &alice, &bob, -1, false),
            ("receiver not whitelisted", &alice, &dave, 100, false),
            ("sender not whitelisted", &dave, &alice, 100, false),
        ] {
            let actual = c.check_transfer_eligibility(from, to, &amount);
            assert_eq!(actual, expected, "eligibility mismatch: {desc}");

            let mut o = JsonObj::new();
            o.push("case", Json::str(desc));
            o.push("amount", Json::str(amount.to_string()));
            o.push("returns", h.render(actual));
            cases.push(o.build());
        }

        scenarios.push(
            Scenario::new(
                "check-transfer-eligibility-matrix",
                "The pre-flight transfer check across boundary amounts and compliance \
                 states. This is a point-in-time read: state can change before the \
                 transfer is submitted, so SDKs must still handle a revert.",
            )
            .set("call", Json::str("check_transfer_eligibility"))
            .set("sender_balance", h.render(c.get_balance_of(&alice)))
            .set("cases", Json::Arr(cases))
            .build(),
        );
    }

    assert_unique_ids(&scenarios);
    write_or_verify(
        "03-transfers.json",
        &envelope(
            "03-transfers",
            "Transfer flows and the read-only eligibility helpers that predict them.",
            scenarios,
        ),
    );
}

// ─── 04 · Events ──────────────────────────────────────────────────────────────

/// One canonical example of every event topic the contract can emit, so
/// indexers can pin topic strings and payload field names.
#[test]
fn fixture_events() {
    let mut scenarios = Vec::new();

    let mut push_event = |id: &str, description: &str, events: Json| {
        scenarios.push(Scenario::new(id, description).set("events", events).build());
    };

    // Compliance events.
    {
        let h = bootstrap();
        let c = h.client();
        c.whitelist_user(&h.actor("compliance_officer"), &h.actor("investor_carol"));
        push_event(
            "event-user-whitelisted",
            "Topic `user_whitelisted`, emitted by `whitelist_user`.",
            h.events(),
        );

        c.revoke_whitelist(&h.actor("compliance_officer"), &h.actor("investor_carol"));
        push_event(
            "event-whitelist-revoked",
            "Topic `whitelist_revoked`, emitted by `revoke_whitelist`.",
            h.events(),
        );
    }

    // Asset events.
    {
        let h = bootstrap();
        let c = h.client();
        c.mint_asset(
            &h.actor("asset_manager"),
            &h.actor("investor_alice"),
            &1_000,
        );
        push_event(
            "event-asset-minted",
            "Topic `asset_minted`. `total_supply` is cumulative across all holders.",
            h.events(),
        );

        c.transfer(&h.actor("investor_alice"), &h.actor("investor_bob"), &250);
        push_event(
            "event-transfer",
            "Topic `transfer`, emitted on every successful transfer.",
            h.events(),
        );

        c.distribute_yield(&h.actor("asset_manager"), &500);
        push_event(
            "event-yield-distributed",
            "Topic `yield_distributed`. The current implementation is a mock that \
             emits the event without moving balances (see asset.rs).",
            h.events(),
        );
    }

    // Admin & role events.
    {
        let h = Harness::new();
        let c = h.client();
        c.initialize(&h.actor("admin"));

        c.set_role(
            &h.actor("admin"),
            &h.actor("compliance_officer"),
            &Role::ComplianceOfficer,
        );
        push_event(
            "event-role-assigned",
            "Topic `role_assigned`. The `role` field is a unit enum, encoded as a \
             single-element vector holding the variant name.",
            h.events(),
        );

        c.remove_role(&h.actor("admin"), &h.actor("compliance_officer"));
        push_event(
            "event-role-revoked",
            "Topic `role_revoked`. `role` carries the *previous* role, not None.",
            h.events(),
        );

        c.transfer_admin(&h.actor("admin"), &h.actor("investor_carol"));
        push_event(
            "event-admin-transfer-initiated",
            "Topic `admin_transfer_initiated`, step 1 of the 2-step admin handoff.",
            h.events(),
        );

        c.accept_admin(&h.actor("investor_carol"));
        push_event(
            "event-admin-transferred",
            "Topic `admin_transferred`, step 2. The candidate must call `accept_admin`.",
            h.events(),
        );

        // `renounce_admin` reuses the `AdminTransferredEvent` payload but
        // publishes it under the distinct `admin_renounced` topic, with
        // `new_admin` equal to `previous_admin`. Indexers that key only off the
        // payload shape would otherwise mistake this for a normal handoff.
        c.renounce_admin(&h.actor("investor_carol"));
        push_event(
            "event-admin-renounced",
            "Topic `admin_renounced`, emitted by the irreversible `renounce_admin`. \
             It reuses the AdminTransferredEvent payload with `new_admin` equal to \
             `previous_admin`, so the topic — not the payload — is what distinguishes \
             a renounce from a transfer. After this the contract has no admin.",
            h.events(),
        );
    }

    // Pause events.
    {
        let h = bootstrap();
        let c = h.client();
        c.pause(&h.actor("emergency_officer"));
        push_event(
            "event-contract-paused",
            "Topic `contract_paused`. An EmergencyOfficer may pause; only the admin \
             may unpause.",
            h.events(),
        );

        c.unpause(&h.actor("admin"));
        push_event(
            "event-contract-unpaused",
            "Topic `contract_unpaused`, emitted by the admin-only `unpause`.",
            h.events(),
        );
    }

    // Cap governance events.
    {
        let h = bootstrap();
        let c = h.client();
        let admin = h.actor("admin");

        c.propose_supply_cap(&admin, &10_000);
        push_event(
            "event-supply-cap-proposed",
            "Topic `supply_cap_proposed`, step 1 of supply cap governance.",
            h.events(),
        );

        c.accept_supply_cap(&admin);
        push_event(
            "event-supply-cap-amended",
            "Topic `supply_cap_amended`, step 2 — the cap is now enforced.",
            h.events(),
        );

        c.propose_holding_cap(&admin, &2_000);
        push_event(
            "event-holding-cap-proposed",
            "Topic `holding_cap_proposed`, step 1 of holding cap governance.",
            h.events(),
        );

        c.accept_holding_cap(&admin);
        push_event(
            "event-holding-cap-amended",
            "Topic `holding_cap_amended`, step 2 — the per-investor cap is now enforced.",
            h.events(),
        );
    }

    // Asset lifecycle & metadata events.
    {
        let h = bootstrap();
        let c = h.client();

        c.set_asset_status(&h.actor("emergency_officer"), &AssetStatus::Paused);
        push_event(
            "event-asset-status-changed",
            "Topic `asset_status_changed`. Both statuses are unit enums encoded as \
             single-element vectors.",
            h.events(),
        );

        c.set_asset_status(&h.actor("emergency_officer"), &AssetStatus::Active);
        c.update_asset_metadata(
            &h.actor("asset_manager"),
            &SorobanString::from_str(&h.env, "Aegis Sample Tower"),
            &SorobanString::from_str(&h.env, "AST"),
            &SorobanString::from_str(&h.env, "https://example.invalid/aegis/sample-tower.json"),
        );
        push_event(
            "event-asset-metadata-updated",
            "Topic `asset_metadata_updated`. The URI is a documentation-only \
             `example.invalid` host and resolves nowhere.",
            h.events(),
        );
    }

    // Reverted invocations emit nothing.
    {
        let h = bootstrap();
        let c = h.client();
        let alice = h.actor("investor_alice");
        let dave = h.actor("outsider_dave");

        let result = c.try_transfer(&alice, &dave, &100);
        assert_eq!(result, Err(Ok(Error::ReceiverNotWhitelisted)));
        let events = h.events();
        assert_eq!(events, Json::Arr(vec![]));

        scenarios.push(
            Scenario::new(
                "event-none-on-reverted-transfer",
                "Soroban discards all events from a reverted invocation, so a \
                 compliance-blocked transfer emits nothing at all. The numeric error \
                 code is the only off-chain-observable signal — indexers must watch \
                 failed transaction results, not events, to audit blocked transfers \
                 (see docs/events.md).",
            )
            .set("call", Json::str("transfer"))
            .set("result", err_json(Error::ReceiverNotWhitelisted))
            .set("events", events)
            .build(),
        );
    }

    assert_unique_ids(&scenarios);

    // Coverage guard: every topic the contract can publish must have a
    // captured example here. The expected list is derived by scanning the
    // contract source for `publish(("topic",), …)`, so adding a new event
    // without a fixture fails this test rather than silently leaving
    // downstream indexers without a reference payload.
    let emitted = topics_emitted_by_contract();
    let captured = topics_in(&scenarios);
    let missing: Vec<&String> = emitted.iter().filter(|t| !captured.contains(*t)).collect();
    assert!(
        missing.is_empty(),
        "these event topics are emitted by the contract but have no fixture \
         example: {missing:?}\nAdd a scenario for each in `fixture_events`."
    );

    write_or_verify(
        "04-events.json",
        &envelope(
            "04-events",
            "One canonical example of every event topic the contract emits.",
            scenarios,
        ),
    );
}

/// Scans the contract source for every `env.events().publish(("topic",), …)`
/// call and returns the set of topic strings.
///
/// Reading the source keeps this guard honest without a runtime registry: the
/// contract has no way to enumerate its own topics, so the source is the only
/// complete list.
fn topics_emitted_by_contract() -> std::collections::BTreeSet<String> {
    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut topics = std::collections::BTreeSet::new();

    for entry in std::fs::read_dir(&src_dir).expect("src/ must be readable") {
        let path = entry.expect("readable entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let body = std::fs::read_to_string(&path).expect("source readable");

        // Match `publish(` then the first quoted string that follows, tolerating
        // the line breaks rustfmt introduces inside the call.
        let mut rest = body.as_str();
        while let Some(idx) = rest.find(".publish(") {
            rest = &rest[idx + ".publish(".len()..];
            let head: String = rest.chars().take(120).collect();
            if let Some(open) = head.find('"') {
                if let Some(len) = head[open + 1..].find('"') {
                    let topic = &head[open + 1..open + 1 + len];
                    if !topic.is_empty()
                        && topic.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
                    {
                        topics.insert(topic.to_string());
                    }
                }
            }
        }
    }

    assert!(
        topics.len() >= 17,
        "topic scan found only {} topics, which suggests the scanner broke rather \
         than that the contract shrank: {topics:?}",
        topics.len()
    );
    topics
}

/// Collects every `topic` value appearing in a set of rendered scenarios.
fn topics_in(scenarios: &[Json]) -> std::collections::BTreeSet<String> {
    fn walk(node: &Json, out: &mut std::collections::BTreeSet<String>) {
        match node {
            Json::Obj(fields) => {
                for (k, v) in fields {
                    if k == "topic" {
                        if let Json::Str(s) = v {
                            out.insert(s.clone());
                        }
                    }
                    walk(v, out);
                }
            }
            Json::Arr(items) => items.iter().for_each(|i| walk(i, out)),
            _ => {}
        }
    }

    let mut out = std::collections::BTreeSet::new();
    scenarios.iter().for_each(|s| walk(s, &mut out));
    out
}

// ─── 05 · Errors ──────────────────────────────────────────────────────────────

/// Every reachable contract error code, captured from a real failing call.
#[test]
fn fixture_errors() {
    let mut scenarios = Vec::new();

    let mut push_err = |id: &str, description: &str, call: &str, result: Json| {
        scenarios.push(
            Scenario::new(id, description)
                .set("call", Json::str(call))
                .set("result", result)
                .build(),
        );
    };

    // 1000 — AlreadyInitialized.
    {
        let h = Harness::new();
        let c = h.client();
        c.initialize(&h.actor("admin"));
        let r = c.try_initialize(&h.actor("admin"));
        push_err(
            "error-1000-already-initialized",
            "`initialize` on a contract that already has an admin. Indicates an \
             integration bug, not user error.",
            "initialize",
            expect_err(r, Error::AlreadyInitialized),
        );
    }

    // 2000 — NotInitialized.
    {
        let h = Harness::new();
        let c = h.client();
        // Deliberately skip `initialize`: the admin lookup fails.
        let r = c.try_set_role(
            &h.actor("admin"),
            &h.actor("investor_carol"),
            &Role::ComplianceOfficer,
        );
        push_err(
            "error-2000-not-initialized",
            "Any call that needs the admin on a contract that was never initialized. \
             Raised via `panic_with_error!`, so it still carries the contract code 2000.",
            "set_role",
            expect_err(r, Error::NotInitialized),
        );
    }

    // 2001 — NoPendingAdminTransfer.
    {
        let h = bootstrap();
        let c = h.client();
        let r = c.try_accept_admin(&h.actor("investor_carol"));
        push_err(
            "error-2001-no-pending-admin-transfer",
            "`accept_admin` with no `transfer_admin` in flight.",
            "accept_admin",
            expect_err(r, Error::NoPendingAdminTransfer),
        );
    }

    // 3000 — Unauthorized (mint without role).
    {
        let h = bootstrap();
        let c = h.client();
        let r = c.try_mint_asset(&h.actor("investor_alice"), &h.actor("investor_bob"), &100);
        push_err(
            "error-3000-unauthorized-mint",
            "A roleless investor attempts to mint. Surface as a generic permission \
             message; do not leak role internals to end users.",
            "mint_asset",
            expect_err(r, Error::Unauthorized),
        );
    }

    // 3000 — Unauthorized (compliance officer cannot mint).
    {
        let h = bootstrap();
        let c = h.client();
        let r = c.try_mint_asset(
            &h.actor("compliance_officer"),
            &h.actor("investor_bob"),
            &100,
        );
        push_err(
            "error-3000-unauthorized-wrong-role",
            "Roles are scoped, not hierarchical: a ComplianceOfficer cannot mint.",
            "mint_asset",
            expect_err(r, Error::Unauthorized),
        );
    }

    // 3001 — CannotAssignAdminRole.
    {
        let h = bootstrap();
        let c = h.client();
        let r = c.try_set_role(&h.actor("admin"), &h.actor("investor_carol"), &Role::Admin);
        push_err(
            "error-3001-cannot-assign-admin-role",
            "`set_role` refuses Role::Admin; the 2-step `transfer_admin` flow must be \
             used instead.",
            "set_role",
            expect_err(r, Error::CannotAssignAdminRole),
        );
    }

    // 3002 — NoRoleToRevoke.
    {
        let h = bootstrap();
        let c = h.client();
        let r = c.try_remove_role(&h.actor("admin"), &h.actor("outsider_dave"));
        push_err(
            "error-3002-no-role-to-revoke",
            "`remove_role` on an address that has no role assigned.",
            "remove_role",
            expect_err(r, Error::NoRoleToRevoke),
        );
    }

    // 3003 — NotPendingCandidate.
    {
        let h = bootstrap();
        let c = h.client();
        c.transfer_admin(&h.actor("admin"), &h.actor("investor_carol"));
        let r = c.try_accept_admin(&h.actor("outsider_dave"));
        push_err(
            "error-3003-not-pending-candidate",
            "Only the recorded candidate may accept an admin transfer.",
            "accept_admin",
            expect_err(r, Error::NotPendingCandidate),
        );
    }

    // 3004 — ContractPaused.
    {
        let h = bootstrap();
        let c = h.client();
        c.pause(&h.actor("emergency_officer"));
        let r = c.try_transfer(&h.actor("investor_alice"), &h.actor("investor_bob"), &10);
        push_err(
            "error-3004-contract-paused",
            "All state-changing calls are blocked while paused. Reads stay available.",
            "transfer",
            expect_err(r, Error::ContractPaused),
        );
    }

    // 3005 — AlreadyPaused.
    {
        let h = bootstrap();
        let c = h.client();
        c.pause(&h.actor("emergency_officer"));
        let r = c.try_pause(&h.actor("emergency_officer"));
        push_err(
            "error-3005-already-paused",
            "`pause` called while already paused.",
            "pause",
            expect_err(r, Error::AlreadyPaused),
        );
    }

    // 3006 — NotPaused.
    {
        let h = bootstrap();
        let c = h.client();
        let r = c.try_unpause(&h.actor("admin"));
        push_err(
            "error-3006-not-paused",
            "`unpause` called while the contract is running normally.",
            "unpause",
            expect_err(r, Error::NotPaused),
        );
    }

    // 4000 — SenderNotWhitelisted.
    {
        let h = bootstrap();
        let c = h.client();
        let r = c.try_transfer(&h.actor("outsider_dave"), &h.actor("investor_alice"), &100);
        push_err(
            "error-4000-sender-not-whitelisted",
            "The sending address has not completed compliance verification. Prompt the \
             KYC flow rather than showing a raw error.",
            "transfer",
            expect_err(r, Error::SenderNotWhitelisted),
        );
    }

    // 4001 — ReceiverNotWhitelisted (transfer).
    {
        let h = bootstrap();
        let c = h.client();
        let r = c.try_transfer(&h.actor("investor_alice"), &h.actor("outsider_dave"), &100);
        push_err(
            "error-4001-receiver-not-whitelisted-transfer",
            "The receiving address is not whitelisted. This is the canonical \
             compliance-restricted transfer signal.",
            "transfer",
            expect_err(r, Error::ReceiverNotWhitelisted),
        );
    }

    // 4001 — ReceiverNotWhitelisted (mint).
    {
        let h = bootstrap();
        let c = h.client();
        let r = c.try_mint_asset(&h.actor("asset_manager"), &h.actor("outsider_dave"), &100);
        push_err(
            "error-4001-receiver-not-whitelisted-mint",
            "Minting to a non-whitelisted address fails with the same code as a \
             transfer, so SDKs can share one handler.",
            "mint_asset",
            expect_err(r, Error::ReceiverNotWhitelisted),
        );
    }

    // 5000 — InvalidAmount (transfer and mint).
    {
        let h = bootstrap();
        let c = h.client();
        let r = c.try_transfer(&h.actor("investor_alice"), &h.actor("investor_bob"), &0);
        push_err(
            "error-5000-invalid-amount-transfer",
            "Amounts must be strictly greater than zero.",
            "transfer",
            expect_err(r, Error::InvalidAmount),
        );

        let r = c.try_mint_asset(&h.actor("asset_manager"), &h.actor("investor_alice"), &-5);
        push_err(
            "error-5000-invalid-amount-mint-negative",
            "A negative mint amount is rejected before any balance is touched.",
            "mint_asset",
            expect_err(r, Error::InvalidAmount),
        );
    }

    // 5001 — InsufficientBalance.
    {
        let h = bootstrap();
        let c = h.client();
        c.mint_asset(&h.actor("asset_manager"), &h.actor("investor_alice"), &100);
        let r = c.try_transfer(&h.actor("investor_alice"), &h.actor("investor_bob"), &101);
        push_err(
            "error-5001-insufficient-balance",
            "Transferring more than the sender holds. Note this is checked *after* \
             compliance, so a non-whitelisted sender sees 4000 instead.",
            "transfer",
            expect_err(r, Error::InsufficientBalance),
        );
    }

    // 6000 — AssetNotActive.
    {
        let h = bootstrap();
        let c = h.client();
        c.set_asset_status(&h.actor("emergency_officer"), &AssetStatus::Paused);
        let r = c.try_mint_asset(&h.actor("asset_manager"), &h.actor("investor_alice"), &100);
        push_err(
            "error-6000-asset-not-active",
            "The asset lifecycle status is not Active, so issuance and transfers are \
             blocked. Distinct from the global contract pause (3004).",
            "mint_asset",
            expect_err(r, Error::AssetNotActive),
        );
    }

    // 6001 — InvalidAssetStatusTransition.
    {
        let h = bootstrap();
        let c = h.client();
        c.set_asset_status(&h.actor("emergency_officer"), &AssetStatus::Retired);
        let r = c.try_set_asset_status(&h.actor("emergency_officer"), &AssetStatus::Active);
        push_err(
            "error-6001-invalid-asset-status-transition",
            "Retired is terminal: no transition out of it is permitted.",
            "set_asset_status",
            expect_err(r, Error::InvalidAssetStatusTransition),
        );
    }

    // 6002 — AssetMetadataUpdateBlocked.
    {
        let h = bootstrap();
        let c = h.client();
        c.set_asset_status(&h.actor("emergency_officer"), &AssetStatus::Retired);
        let r = c.try_update_asset_metadata(
            &h.actor("asset_manager"),
            &SorobanString::from_str(&h.env, "Renamed"),
            &SorobanString::from_str(&h.env, "RNM"),
            &SorobanString::from_str(&h.env, "https://example.invalid/renamed.json"),
        );
        push_err(
            "error-6002-asset-metadata-update-blocked",
            "Metadata is frozen once the asset is Retired or Blocked.",
            "update_asset_metadata",
            expect_err(r, Error::AssetMetadataUpdateBlocked),
        );
    }

    // Host trap — supply cap exceeded.
    {
        let h = bootstrap();
        let c = h.client();
        let admin = h.actor("admin");
        c.propose_supply_cap(&admin, &1_000);
        c.accept_supply_cap(&admin);

        let r = c.try_mint_asset(
            &h.actor("asset_manager"),
            &h.actor("investor_alice"),
            &1_001,
        );
        assert!(
            matches!(r, Err(Err(_))),
            "supply cap breach must surface as a host trap, got {r:?}"
        );
        push_err(
            "error-host-trap-supply-cap-exceeded",
            "The supply cap is enforced with an `assert!`, which traps in the host \
             rather than returning a contract error code. SDKs must fail safe here: \
             there is no numeric code to match on.",
            "mint_asset",
            host_error("Mint would exceed the active supply cap"),
        );
    }

    // Host trap — holding cap exceeded.
    {
        let h = bootstrap();
        let c = h.client();
        let admin = h.actor("admin");
        c.propose_holding_cap(&admin, &500);
        c.accept_holding_cap(&admin);

        let r = c.try_mint_asset(&h.actor("asset_manager"), &h.actor("investor_alice"), &501);
        assert!(
            matches!(r, Err(Err(_))),
            "holding cap breach must surface as a host trap, got {r:?}"
        );
        push_err(
            "error-host-trap-holding-cap-exceeded",
            "The per-investor holding cap also traps in the host. Use \
             `get_investor_eligibility` / `check_transfer_eligibility` to detect this \
             *before* submitting, since the failure carries no contract code.",
            "mint_asset",
            host_error("Transfer would exceed the investor holding cap"),
        );
    }

    assert_unique_ids(&scenarios);

    // Coverage guard: every variant of `Error` must have a captured example.
    // This keeps the fixture set honest as the contract grows — adding a new
    // error variant without documenting it fails here rather than leaving the
    // SDK to guess at an undocumented code.
    //
    // The list is exhaustive by construction: `error_name` is a total match
    // over `Error`, so a new variant cannot compile without being added there.
    const ALL_ERRORS: &[Error] = &[
        Error::AlreadyInitialized,
        Error::NotInitialized,
        Error::NoPendingAdminTransfer,
        Error::Unauthorized,
        Error::CannotAssignAdminRole,
        Error::NoRoleToRevoke,
        Error::NotPendingCandidate,
        Error::ContractPaused,
        Error::AlreadyPaused,
        Error::NotPaused,
        Error::SenderNotWhitelisted,
        Error::ReceiverNotWhitelisted,
        Error::InvalidAmount,
        Error::InsufficientBalance,
        Error::AssetNotActive,
        Error::InvalidAssetStatusTransition,
        Error::AssetMetadataUpdateBlocked,
    ];

    let captured: Vec<u32> = scenarios
        .iter()
        .filter_map(|s| match s {
            Json::Obj(fields) => fields.iter().find_map(|(k, v)| match (k.as_str(), v) {
                ("result", Json::Obj(result)) => {
                    result.iter().find_map(|(rk, rv)| match (rk.as_str(), rv) {
                        ("error", Json::Obj(err)) => {
                            err.iter().find_map(|(ek, ev)| match (ek.as_str(), ev) {
                                ("code", Json::Num(n)) => Some(*n as u32),
                                _ => None,
                            })
                        }
                        _ => None,
                    })
                }
                _ => None,
            }),
            _ => None,
        })
        .collect();

    let missing: Vec<&str> = ALL_ERRORS
        .iter()
        .copied()
        .filter(|e| !captured.contains(&(*e as u32)))
        .map(error_name)
        .collect();

    assert!(
        missing.is_empty(),
        "these Error variants have no fixture example: {missing:?}\n\
         Add a scenario for each in `fixture_errors` so SDK consumers get a \
         known-good example of every code."
    );

    write_or_verify(
        "05-errors.json",
        &envelope(
            "05-errors",
            "Every reachable error, captured from a real failing call, with its stable \
             numeric code and category.",
            scenarios,
        ),
    );
}

// ─── 06 · Capability reads ────────────────────────────────────────────────────

/// The read-only surface an SDK polls to render state: balances, supply, caps,
/// pause state, asset status, and metadata.
#[test]
fn fixture_capabilities() {
    let mut scenarios = Vec::new();

    // 1 — pristine contract defaults.
    {
        let h = Harness::new();
        let c = h.client();
        c.initialize(&h.actor("admin"));

        let mut reads = JsonObj::new();
        reads.push("get_total_supply", h.render(c.get_total_supply()));
        reads.push("get_supply_cap", h.render(c.get_supply_cap()));
        reads.push(
            "get_pending_supply_cap",
            h.render(c.get_pending_supply_cap()),
        );
        reads.push("get_holding_cap", h.render(c.get_holding_cap()));
        reads.push(
            "get_pending_holding_cap",
            h.render(c.get_pending_holding_cap()),
        );
        reads.push("is_paused", h.render(c.is_paused()));
        reads.push("get_asset_status", h.render(c.get_asset_status()));
        reads.push("get_asset_metadata", h.render(c.get_asset_metadata()));
        reads.push(
            "get_balance_of(outsider_dave)",
            h.render(c.get_balance_of(&h.actor("outsider_dave"))),
        );
        reads.push(
            "is_whitelisted(outsider_dave)",
            h.render(c.is_whitelisted(&h.actor("outsider_dave"))),
        );

        scenarios.push(
            Scenario::new(
                "reads-freshly-initialized-contract",
                "Every read on a freshly initialized contract. Caps default to 0 \
                 (unbounded/unrestricted), pending caps are null, the asset defaults to \
                 Active, and metadata strings are empty until set.",
            )
            .set("state", Json::str("initialized only"))
            .set("reads", reads.build())
            .build(),
        );
    }

    // 2 — fully configured contract.
    {
        let h = bootstrap();
        let c = h.client();
        let admin = h.actor("admin");
        let manager = h.actor("asset_manager");
        let alice = h.actor("investor_alice");
        let bob = h.actor("investor_bob");

        c.propose_supply_cap(&admin, &1_000_000);
        c.accept_supply_cap(&admin);
        c.propose_holding_cap(&admin, &600_000);
        c.accept_holding_cap(&admin);
        c.update_asset_metadata(
            &manager,
            &SorobanString::from_str(&h.env, "Aegis Sample Tower"),
            &SorobanString::from_str(&h.env, "AST"),
            &SorobanString::from_str(&h.env, "https://example.invalid/aegis/sample-tower.json"),
        );
        c.mint_asset(&manager, &alice, &250_000);
        c.mint_asset(&manager, &bob, &100_000);
        c.transfer(&alice, &bob, &50_000);

        let mut reads = JsonObj::new();
        reads.push("get_total_supply", h.render(c.get_total_supply()));
        reads.push("get_supply_cap", h.render(c.get_supply_cap()));
        reads.push("get_holding_cap", h.render(c.get_holding_cap()));
        reads.push("is_paused", h.render(c.is_paused()));
        reads.push("get_asset_status", h.render(c.get_asset_status()));
        reads.push("get_asset_metadata", h.render(c.get_asset_metadata()));

        let mut balances = JsonObj::new();
        balances.push("investor_alice", h.render(c.get_balance_of(&alice)));
        balances.push("investor_bob", h.render(c.get_balance_of(&bob)));
        reads.push("balances", balances.build());

        let mut elig = JsonObj::new();
        elig.push(
            "investor_alice",
            h.render(c.get_investor_eligibility(&alice)),
        );
        elig.push("investor_bob", h.render(c.get_investor_eligibility(&bob)));
        reads.push("eligibility", elig.build());

        assert_eq!(c.get_total_supply(), 350_000);
        assert_eq!(c.get_balance_of(&alice), 200_000);
        assert_eq!(c.get_balance_of(&bob), 150_000);

        scenarios.push(
            Scenario::new(
                "reads-fully-configured-contract",
                "A realistic configured state: caps set, metadata populated, two \
                 holders, one transfer settled. This is the reference state for \
                 dashboard rendering tests.",
            )
            .set(
                "setup",
                Json::Arr(vec![
                    Json::str("supply cap 1000000, holding cap 600000"),
                    Json::str("mint 250000 to alice, 100000 to bob"),
                    Json::str("transfer 50000 alice -> bob"),
                ]),
            )
            .set("reads", reads.build())
            .build(),
        );
    }

    // 3 — reads remain available while paused.
    {
        let h = bootstrap();
        let c = h.client();
        c.mint_asset(
            &h.actor("asset_manager"),
            &h.actor("investor_alice"),
            &1_000,
        );
        c.pause(&h.actor("emergency_officer"));

        let alice = h.actor("investor_alice");
        let mut reads = JsonObj::new();
        reads.push("is_paused", h.render(c.is_paused()));
        reads.push("get_total_supply", h.render(c.get_total_supply()));
        reads.push("get_balance_of", h.render(c.get_balance_of(&alice)));
        reads.push("is_whitelisted", h.render(c.is_whitelisted(&alice)));
        reads.push(
            "get_investor_eligibility",
            h.render(c.get_investor_eligibility(&alice)),
        );
        reads.push(
            "check_transfer_eligibility",
            h.render(c.check_transfer_eligibility(&alice, &h.actor("investor_bob"), &10)),
        );

        scenarios.push(
            Scenario::new(
                "reads-while-paused",
                "Reads stay callable during a pause, but every eligibility flag goes \
                 false because no state-changing call can succeed. Dashboards should \
                 show a maintenance banner driven by `is_paused` rather than treating \
                 the false flags as a compliance failure.",
            )
            .set("state", Json::str("paused"))
            .set("reads", reads.build())
            .build(),
        );
    }

    // 4 — asset lifecycle status values.
    {
        let h = bootstrap();
        let c = h.client();
        let officer = h.actor("emergency_officer");

        let mut statuses = Vec::new();
        let mut record = |label: &str, value: Json| {
            let mut o = JsonObj::new();
            o.push("status", Json::str(label));
            o.push("get_asset_status", value);
            statuses.push(o.build());
        };

        record("Active (default)", h.render(c.get_asset_status()));
        c.set_asset_status(&officer, &AssetStatus::Paused);
        record("Paused", h.render(c.get_asset_status()));
        c.set_asset_status(&officer, &AssetStatus::Blocked);
        record("Blocked", h.render(c.get_asset_status()));
        c.set_asset_status(&officer, &AssetStatus::Retired);
        record("Retired (terminal)", h.render(c.get_asset_status()));

        scenarios.push(
            Scenario::new(
                "reads-asset-status-values",
                "Every value `get_asset_status` can return, in a valid transition \
                 order. Retired is terminal.",
            )
            .set("call", Json::str("get_asset_status"))
            .set("values", Json::Arr(statuses))
            .build(),
        );
    }

    assert_unique_ids(&scenarios);
    write_or_verify(
        "06-capabilities.json",
        &envelope(
            "06-capabilities",
            "Read-only capability surface: balances, supply, caps, pause, status, metadata.",
            scenarios,
        ),
    );
}

// ─── Harness self-checks ──────────────────────────────────────────────────────

/// The fixtures are only useful if they are reproducible. This asserts the
/// harness itself is deterministic: two independent runs of the same scenario
/// must produce identical output.
#[test]
fn fixtures_are_deterministic() {
    let render_once = || {
        let h = bootstrap();
        let c = h.client();
        c.mint_asset(
            &h.actor("asset_manager"),
            &h.actor("investor_alice"),
            &1_000,
        );
        let mint_events = h.events();
        c.transfer(&h.actor("investor_alice"), &h.actor("investor_bob"), &250);
        let transfer_events = h.events();
        let elig = h.render(c.get_investor_eligibility(&h.actor("investor_alice")));

        Json::Arr(vec![mint_events, transfer_events, elig]).to_pretty()
    };

    assert_eq!(
        render_once(),
        render_once(),
        "fixture generation must be byte-for-byte reproducible"
    );
}

/// Guards the privacy requirement: no fixture may contain a real-looking
/// identity, network endpoint, or personal data.
#[test]
fn fixtures_contain_no_real_user_data() {
    // Every address the fixtures are allowed to mention.
    let allowed: Vec<&str> = ACTORS
        .iter()
        .map(|(_, k)| *k)
        .chain(std::iter::once(CONTRACT_ADDRESS))
        .collect();

    // Terms that would suggest real personal data or a live endpoint leaked in.
    //
    // These are matched against the *human-readable* portion of each fixture
    // only. The base64 `xdr_base64` blobs are excluded first: they are
    // machine-generated wire bytes whose alphabet can reproduce almost any
    // short letter sequence by chance, which would make a naive substring
    // scan flaky rather than meaningful.
    const BANNED: &[&str] = &[
        "@gmail",
        "@yahoo",
        "@outlook",
        "@hotmail",
        "passport",
        "ssn",
        "national id",
        "date of birth",
        "horizon.stellar.org",
        "soroban-testnet.stellar.org",
        "mainnet",
        "pubnet",
    ];

    /// Removes every `"xdr_base64": "..."` value from the text so the scans
    /// below only see human-meaningful content.
    fn strip_xdr(body: &str) -> String {
        const KEY: &str = "\"xdr_base64\":";
        let mut out = String::with_capacity(body.len());
        let mut rest = body;
        while let Some(idx) = rest.find(KEY) {
            out.push_str(&rest[..idx]);
            rest = &rest[idx + KEY.len()..];
            // Skip to the end of the quoted value that follows.
            if let Some(open) = rest.find('"') {
                if let Some(close) = rest[open + 1..].find('"') {
                    rest = &rest[open + 1 + close + 1..];
                    continue;
                }
            }
            break;
        }
        out.push_str(rest);
        out
    }

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(support::FIXTURE_DIR);
    let entries = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("fixtures dir {} unreadable: {e}", dir.display()));

    let mut checked = 0;
    for entry in entries {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        checked += 1;
        let body = std::fs::read_to_string(&path).expect("fixture readable");
        let prose = strip_xdr(&body);
        let lower = prose.to_lowercase();

        for term in BANNED {
            assert!(
                !lower.contains(&term.to_lowercase()),
                "{} contains banned term {term:?}",
                path.display()
            );
        }

        // No Stellar secret seed (S…) may ever appear, even in the XDR blobs.
        // Secret seeds are strkeys over the base32 alphabet, so restricting to
        // that alphabet avoids flagging ordinary base64 runs.
        let secret_like = body.split(|c: char| !c.is_ascii_alphanumeric()).find(|t| {
            t.len() == 56
                && t.starts_with('S')
                && t.bytes()
                    .all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b))
        });
        assert!(
            secret_like.is_none(),
            "{} contains something shaped like a Stellar secret key: {:?}",
            path.display(),
            secret_like
        );

        // Any G…/C… strkey in the prose must be one of the synthetic actors.
        for token in prose
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|t| t.len() == 56 && (t.starts_with('G') || t.starts_with('C')))
        {
            assert!(
                allowed.contains(&token),
                "{} contains unrecognised address {token} — every address must come \
                 from the synthetic actor table",
                path.display()
            );
        }

        // Only the documentation-reserved host may appear.
        for token in body.split('"') {
            if token.contains("://") {
                assert!(
                    token.contains("example.invalid"),
                    "{} references a non-documentation URL: {token}",
                    path.display()
                );
            }
        }
    }

    assert!(
        checked >= 7,
        "expected at least 7 fixture files, found {checked}"
    );
}
