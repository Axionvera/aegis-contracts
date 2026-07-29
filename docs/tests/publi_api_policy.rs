//! Documentation contract tests.
//!
//! These intentionally couple the exported contract surface to its normative
//! reference. An API/event change must update `docs/public-api.md` in the same
//! pull request, which prevents silent drift for SDK and dashboard consumers.

const POLICY: &str = include_str!("../docs/public-api.md");
const README: &str = include_str!("../README.md");
const CONTRIBUTING: &str = include_str!("../CONTRIBUTING.md");
const CONTRACT_SPEC: &str = include_str!("../docs/contract-spec.md");

const CONTRACT_SOURCES: [&str; 3] = [
    include_str!("../src/lib.rs"),
    include_str!("../src/compliance.rs"),
    include_str!("../src/asset.rs"),
];
const EVENT_SOURCE: &str = include_str!("../src/events.rs");

fn contract_entrypoints() -> Vec<&'static str> {
    CONTRACT_SOURCES
        .iter()
        .flat_map(|source| source.lines())
        // Exported methods inside `impl AegisContract` have one indentation
        // level. Module-level Rust helpers deliberately do not match.
        .filter_map(|line| line.strip_prefix("    pub fn "))
        .map(|signature| {
            signature
                .split_once('(')
                .expect("contract function has an argument list")
                .0
        })
        .collect()
}

fn contract_events() -> Vec<&'static str> {
    EVENT_SOURCE
        .lines()
        .filter_map(|line| line.strip_prefix("pub struct "))
        .map(|declaration| {
            declaration
                .split_once([' ', '{'])
                .map_or(declaration, |(name, _)| name)
        })
        .collect()
}

#[test]
fn every_contract_entrypoint_has_a_public_reference() {
    let entrypoints = contract_entrypoints();
    assert_eq!(
        entrypoints.len(),
        5,
        "unexpected contract entry-point count"
    );

    for entrypoint in entrypoints {
        let heading = format!("### `{entrypoint}`");
        assert!(
            POLICY.contains(&heading),
            "missing public API section for `{entrypoint}`"
        );
    }
}

#[test]
fn every_contract_event_has_a_wire_format_reference() {
    let events = contract_events();
    assert_eq!(events.len(), 5, "unexpected contract event count");

    for event in events {
        let heading = format!("### `{event}`");
        assert!(
            POLICY.contains(&heading),
            "missing event wire-format section for `{event}`"
        );
    }
}

#[test]
fn policy_covers_required_compatibility_areas() {
    for heading in [
        "## Stability classification",
        "## Public function reference",
        "## Error model and failure reference",
        "## Event reference",
        "## Storage implications",
        "## SDK expectations",
        "## Dashboard and indexer expectations",
        "## Breaking-change categories",
        "## Versioning and deprecation",
        "## Change review requirements",
    ] {
        assert!(
            POLICY.contains(heading),
            "missing policy section: {heading}"
        );
    }

    for storage_key in ["Admin", "Whitelist", "Balance", "TotalSupply"] {
        assert!(
            POLICY.contains(&format!("`DataKey::{storage_key}")),
            "missing storage implication for DataKey::{storage_key}"
        );
    }
}

#[test]
fn current_explicit_failures_are_referenced() {
    for diagnostic in [
        "Contract already initialized",
        "Unauthorized: Only admin can whitelist",
        "Unauthorized: Only admin can mint",
        "`Unauthorized`",
        "Amount must be positive",
        "Sender is not whitelisted",
        "Receiver is not whitelisted",
        "Insufficient balance",
    ] {
        assert!(
            POLICY.contains(diagnostic),
            "missing failure reference for: {diagnostic}"
        );
    }
}

#[test]
fn repository_guidance_links_to_the_canonical_policy() {
    let policy_link = "docs/public-api.md";
    assert!(
        README.contains(policy_link),
        "README must link to the API policy"
    );
    assert!(
        CONTRIBUTING.contains(policy_link),
        "contributor guidance must link to the API policy"
    );
    assert!(
        CONTRACT_SPEC.contains("(public-api.md)"),
        "contract spec index must link to the API policy"
    );
}
