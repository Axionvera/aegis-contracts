use soroban_sdk::{contractimpl, contracttype, vec, Env, String, Symbol, Vec};

use crate::admin::is_paused;
use crate::holding::get_holding_cap;
use crate::lifecycle::{get_asset_status, AssetStatus};
use crate::supply_cap::get_supply_cap;
use crate::{AegisContract, AegisContractArgs, AegisContractClient, DataKey};

// ─── Schema version ───────────────────────────────────────────────────────────

/// Schema version of the [`ContractCapabilities`] response.
///
///
/// Bump this whenever a field is **added** to any capability struct, or a
/// capability key is added to the registry, so an SDK pinned to an older
/// schema can detect that the deployed contract may advertise capabilities it
/// does not know about. Fields are append-only: never remove or repurpose an
/// existing field or key (same stability contract as `docs/events.md` topics
/// and `docs/error-codes.md` numeric codes).
pub const CAPABILITY_SCHEMA_VERSION: u32 = 3;

// ─── Response types ───────────────────────────────────────────────────────────

/// Tri-state support marker for a single protocol behaviour.
///
/// A plain `bool` cannot distinguish "this contract will never do that" from
/// "not built yet, but it is a tracked gap". Front-ends need that distinction
/// to decide between hiding a control permanently and rendering a
/// "coming soon" affordance, so every behaviour flag is a tri-state.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapabilityStatus {
    /// Not available in this contract version, and not a tracked gap —
    /// either deliberately out of scope or impossible under the protocol's
    /// design (e.g. transfer-restriction events, which Soroban discards
    /// along with the reverted invocation that would publish them).
    /// Clients must hide the corresponding UI entirely.
    Unsupported,
    /// Not available yet, but a known and documented gap that a future
    /// contract version is expected to close (e.g. SEP-41 allowances).
    /// Clients may render a disabled/"coming soon" control, but must never
    /// build a transaction against it.
    Planned,
    /// Implemented and callable against this deployment right now.
    Supported,
}

/// Compliance module capabilities (`compliance.rs`, `eligibility.rs`).
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComplianceCapabilities {
    /// Whether the compliance module is compiled into this deployment.
    pub module_enabled: bool,
    /// Adding an address to the KYC whitelist (`whitelist_user`).
    pub whitelist: CapabilityStatus,
    /// Removing an address from the KYC whitelist (`revoke_whitelist`).
    pub whitelist_revocation: CapabilityStatus,
    /// Whitelisting many addresses in one invocation.
    pub batch_whitelisting: CapabilityStatus,
    /// Updating many lifecycle statuses in one atomic invocation.
    pub batch_status_updates: CapabilityStatus,
    /// Per-investor jurisdiction / accreditation tiers. The lifecycle models
    /// compliance *state*, not investor class, so regime-specific
    /// segmentation (Reg D vs. Reg S) remains off-chain only.
    pub investor_tiers: CapabilityStatus,
    /// Five-state compliance lifecycle (`Unknown` / `Pending` / `Approved` /
    /// `Revoked` / `Blocked`) with per-address status reads
    /// (`get_compliance_status`). See `docs/compliance-lifecycle.md`.
    pub lifecycle_states: CapabilityStatus,
    /// Enforced transition matrix on every status change
    /// (`set_compliance_status`), plus the pre-flight reads
    /// `is_compliance_transition_allowed` /
    /// `get_allowed_compliance_transitions`.
    pub lifecycle_transitions: CapabilityStatus,
    /// Aggregated read helpers (`get_investor_eligibility`,
    /// `check_transfer_eligibility`).
    pub eligibility_reads: CapabilityStatus,
    /// Whether every mint checks the receiver's lifecycle status.
    pub enforced_on_mint: bool,
    /// Whether every transfer checks both parties' lifecycle statuses.
    pub enforced_on_transfer: bool,
}

/// Minting / issuance capabilities (`asset.rs`, `supply_cap.rs`).
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MintingCapabilities {
    /// Whether the minting module is compiled into this deployment.
    pub module_enabled: bool,
    /// Issuing new units to a whitelisted holder (`mint_asset`).
    pub minting: CapabilityStatus,
    /// Destroying units. No burn entrypoint exists on this contract.
    pub burning: CapabilityStatus,
    /// Global supply cap with 2-step amendment governance.
    pub supply_cap: CapabilityStatus,
    /// Runtime: whether a supply cap is currently active (`> 0`). `false`
    /// means unbounded minting, subject to the compliance whitelist.
    pub supply_cap_enforced: bool,
    /// On-chain yield settlement. `distribute_yield` exists but only emits
    /// `yield_distributed` for off-chain indexing — it moves no value.
    pub yield_distribution: CapabilityStatus,
}

/// Transfer capabilities (`asset.rs`, `holding.rs`, `eligibility.rs`).
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferCapabilities {
    /// Whether the transfer module is compiled into this deployment.
    pub module_enabled: bool,
    /// Direct holder-to-holder transfers (`transfer`).
    pub transfers: CapabilityStatus,
    /// Per-investor holding cap with 2-step amendment governance.
    pub holding_cap: CapabilityStatus,
    /// Runtime: whether a holding cap is currently active (`> 0`). `false`
    /// means any whitelisted balance is permitted.
    pub holding_cap_enforced: bool,
    /// SEP-41 `approve` / `allowance` delegation.
    pub allowances: CapabilityStatus,
    /// SEP-41 `transfer_from` spending on a holder's behalf.
    pub transfer_from: CapabilityStatus,
    /// Fee deduction on transfer.
    pub transfer_fees: CapabilityStatus,
    /// Pre-flight transfer simulation (`check_transfer_eligibility`).
    pub transfer_eligibility_check: CapabilityStatus,
    /// Granular blocked-transfer reason codes
    /// (`check_transfer_restriction` / `check_mint_restriction`), returning a
    /// specific `RestrictionReason` instead of a generic failure. See
    /// `docs/transfer-restrictions.md`.
    pub transfer_restriction_reasons: CapabilityStatus,
}

/// Pause and asset-lifecycle capabilities (`admin.rs`, `asset.rs`).
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PauseCapabilities {
    /// Whether the pause module is compiled into this deployment.
    pub module_enabled: bool,
    /// Global emergency pause (`pause` / `unpause`).
    pub global_pause: CapabilityStatus,
    /// Runtime: whether the contract is currently globally paused.
    pub paused: bool,
    /// Per-asset lifecycle status transitions (`set_asset_status`).
    pub asset_lifecycle: CapabilityStatus,
    /// Runtime: whether the asset lifecycle status is `Active`.
    pub asset_active: bool,
    /// Runtime, derived: `!paused && asset_active`. When `false`, no mint or
    /// transfer can succeed for **any** investor. This is a protocol-level
    /// switch only — an investor may still be individually ineligible while
    /// this is `true` (see `get_investor_eligibility`).
    pub operations_enabled: bool,
}

/// Asset metadata capabilities (`asset.rs`).
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataCapabilities {
    /// Whether the metadata module is compiled into this deployment.
    pub module_enabled: bool,
    /// Readable/writable asset name and ticker symbol.
    pub name_and_symbol: CapabilityStatus,
    /// Off-chain metadata URI pointer.
    pub metadata_uri: CapabilityStatus,
    /// SEP-41 `decimals` precision. Clients must not infer a precision.
    pub decimals: CapabilityStatus,
    /// Runtime: whether a non-empty name **and** symbol have been set. When
    /// `false`, a dashboard should fall back to a placeholder rather than
    /// rendering blank strings.
    pub metadata_configured: bool,
    /// Whether metadata updates are blocked in terminal lifecycle statuses
    /// (`Retired` / `Blocked`).
    pub lifecycle_restricted: bool,
}

/// Global protocol configuration capabilities (`config.rs`).
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigCapabilities {
    /// Whether the global configuration module is compiled into this deployment.
    pub module_enabled: bool,
    /// Protocol configuration 2-step governance.
    pub global_config: CapabilityStatus,
}

/// Event schema capabilities. See `docs/events.md` for topic/payload shapes.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventCapabilities {
    /// Whether this deployment publishes structured `#[contracttype]` events.
    pub module_enabled: bool,
    /// `user_whitelisted`, `whitelist_revoked`.
    pub compliance_events: CapabilityStatus,
    /// `compliance_status_changed` — the canonical lifecycle transition
    /// event, carrying both the previous and the new status.
    pub compliance_lifecycle_events: CapabilityStatus,
    /// `asset_minted`, `yield_distributed`.
    pub minting_events: CapabilityStatus,
    /// `transfer`.
    pub transfer_events: CapabilityStatus,
    /// `role_assigned`, `role_revoked`, `admin_transfer_initiated`,
    /// `admin_transferred`, `contract_paused`, `contract_unpaused`.
    pub admin_events: CapabilityStatus,
    /// `supply_cap_proposed`, `supply_cap_amended`, `holding_cap_proposed`,
    /// `holding_cap_amended`.
    pub governance_events: CapabilityStatus,
    /// `asset_status_changed`, `asset_metadata_updated`.
    pub asset_lifecycle_events: CapabilityStatus,
    /// A durable event for a *blocked* transfer. Structurally impossible:
    /// Soroban discards events from reverted invocations, so indexers must
    /// watch the granular restriction error codes (`3004`, `4000`, `4001`,
    /// `7000`–`7004`) instead — see `transfer_restriction_reasons` and
    /// `docs/transfer-restrictions.md`.
    pub transfer_restriction_events: CapabilityStatus,
    /// A dedicated `asset_registered` event, distinct from the first
    /// `asset_minted` to a holder.
    pub asset_registered_event: CapabilityStatus,
}

/// Read-only capability descriptor for a deployed Aegis contract.
///
/// Aggregates which modules are enabled and which protocol behaviours are
/// supported, so SDK and dashboard clients can feature-gate their UI from a
/// single call instead of hardcoding assumptions per deployment or probing
/// entrypoints and catching reverts. See `docs/capabilities.md`.
///
/// Two kinds of field appear here and must not be conflated:
///
/// * **Static capability** — [`CapabilityStatus`] fields and `module_enabled`
///   booleans. Fixed for a given contract build; safe to cache for the
///   lifetime of a deployment.
/// * **Runtime switch** — the `*_enforced`, `paused`, `asset_active`,
///   `operations_enabled`, `metadata_configured`, and `initialized` booleans.
///   Derived from current ledger state and can change between calls; re-read
///   these rather than caching them.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractCapabilities {
    /// Schema version of this response ([`CAPABILITY_SCHEMA_VERSION`]).
    pub capability_version: u32,
    /// Semantic version of the deployed contract crate.
    pub contract_version: String,
    /// Runtime: whether `initialize` has been called. When `false`, every
    /// privileged entrypoint reverts with `NotInitialized` (2000).
    pub initialized: bool,
    /// Role-based access control (`set_role` / `remove_role` / `get_role_of`).
    pub rbac: CapabilityStatus,
    /// 2-step propose-then-accept governance for admin transfer, supply cap,
    /// and holding cap changes.
    pub two_step_governance: CapabilityStatus,
    /// Conformance with the SEP-41 Soroban token interface as a whole.
    /// `Planned` — `approve`, `allowance`, `transfer_from`, `burn`, and
    /// `decimals` are all missing, so generic wallets and DEX aggregators
    /// cannot treat this contract as a standard token.
    pub sep41_token_interface: CapabilityStatus,
    /// Compliance / KYC whitelist capabilities.
    pub compliance: ComplianceCapabilities,
    /// Minting and supply capabilities.
    pub minting: MintingCapabilities,
    /// Transfer capabilities.
    pub transfers: TransferCapabilities,
    /// Pause and asset-lifecycle capabilities.
    pub pause: PauseCapabilities,
    /// Asset metadata capabilities.
    pub metadata: MetadataCapabilities,
    /// Event schema capabilities.
    pub events: EventCapabilities,
    /// Protocol configuration capabilities.
    pub config: ConfigCapabilities,
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Maps a runtime boolean onto the tri-state marker.
fn status_of(enabled: bool) -> CapabilityStatus {
    if enabled {
        CapabilityStatus::Supported
    } else {
        CapabilityStatus::Unsupported
    }
}

/// Returns whether a non-empty name **and** symbol have been recorded.
///
/// Pure read with a safe default: an uninitialized or never-configured
/// contract reports `false` rather than panicking.
fn is_metadata_configured(env: &Env) -> bool {
    let empty = String::from_str(env, "");
    let name: String = env
        .storage()
        .instance()
        .get(&DataKey::AssetName)
        .unwrap_or(empty.clone());
    let symbol: String = env
        .storage()
        .instance()
        .get(&DataKey::AssetSymbol)
        .unwrap_or(empty);

    !name.is_empty() && !symbol.is_empty()
}

/// Builds the capability descriptor for this deployment.
///
/// Pure read: issues no storage writes, publishes no events, requires no
/// authorization, and never panics — including before `initialize` has been
/// called and while the contract is paused. Every storage lookup falls back
/// to the same safe default its owning module uses.
pub fn get_capabilities(env: &Env) -> ContractCapabilities {
    // ── Runtime switches, read once so the whole response is consistent ──
    let initialized = env.storage().instance().has(&DataKey::Admin);
    let paused = is_paused(env);
    let asset_active = get_asset_status(env) == AssetStatus::Active;
    let supply_cap_enforced = get_supply_cap(env) > 0;
    let holding_cap_enforced = get_holding_cap(env) > 0;
    let metadata_configured = is_metadata_configured(env);

    ContractCapabilities {
        capability_version: CAPABILITY_SCHEMA_VERSION,
        contract_version: String::from_str(env, env!("CARGO_PKG_VERSION")),
        initialized,
        rbac: CapabilityStatus::Supported,
        two_step_governance: CapabilityStatus::Supported,
        // Missing approve/allowance/transfer_from/burn/decimals.
        sep41_token_interface: CapabilityStatus::Planned,

        compliance: ComplianceCapabilities {
            module_enabled: true,
            whitelist: CapabilityStatus::Supported,
            whitelist_revocation: CapabilityStatus::Supported,
            batch_whitelisting: CapabilityStatus::Supported,
            batch_status_updates: CapabilityStatus::Supported,
            // The lifecycle carries compliance state, not investor class.
            investor_tiers: CapabilityStatus::Unsupported,
            lifecycle_states: CapabilityStatus::Supported,
            lifecycle_transitions: CapabilityStatus::Supported,
            eligibility_reads: CapabilityStatus::Supported,
            enforced_on_mint: true,
            enforced_on_transfer: true,
        },

        minting: MintingCapabilities {
            module_enabled: true,
            minting: CapabilityStatus::Supported,
            // No burn entrypoint exists; supply is monotonically increasing.
            burning: CapabilityStatus::Unsupported,
            supply_cap: CapabilityStatus::Supported,
            supply_cap_enforced,
            // Event-only today: no on-chain settlement of yield.
            yield_distribution: CapabilityStatus::Planned,
        },

        transfers: TransferCapabilities {
            module_enabled: true,
            transfers: CapabilityStatus::Supported,
            holding_cap: CapabilityStatus::Supported,
            holding_cap_enforced,
            allowances: CapabilityStatus::Planned,
            transfer_from: CapabilityStatus::Planned,
            transfer_fees: CapabilityStatus::Planned,
            transfer_eligibility_check: CapabilityStatus::Supported,
            transfer_restriction_reasons: CapabilityStatus::Supported,
        },

        pause: PauseCapabilities {
            module_enabled: true,
            global_pause: CapabilityStatus::Supported,
            paused,
            asset_lifecycle: CapabilityStatus::Supported,
            asset_active,
            operations_enabled: !paused && asset_active,
        },

        metadata: MetadataCapabilities {
            module_enabled: true,
            name_and_symbol: CapabilityStatus::Supported,
            metadata_uri: CapabilityStatus::Supported,
            // No decimals slot exists; clients must not infer a precision.
            decimals: CapabilityStatus::Planned,
            metadata_configured,
            lifecycle_restricted: true,
        },

        events: EventCapabilities {
            module_enabled: true,
            compliance_events: CapabilityStatus::Supported,
            compliance_lifecycle_events: CapabilityStatus::Supported,
            minting_events: CapabilityStatus::Supported,
            transfer_events: CapabilityStatus::Supported,
            admin_events: CapabilityStatus::Supported,
            governance_events: CapabilityStatus::Supported,
            asset_lifecycle_events: CapabilityStatus::Supported,
            // Structurally impossible under Soroban's revert semantics.
            transfer_restriction_events: CapabilityStatus::Unsupported,
            asset_registered_event: CapabilityStatus::Planned,
        },
        config: ConfigCapabilities {
            module_enabled: true,
            global_config: CapabilityStatus::Supported,
        },
    }
}

/// Resolves a single capability key to its status.
///
/// Derived from [`get_capabilities`] so the two can never disagree. Unknown
/// keys — including capabilities added in a future contract version that this
/// deployment has never heard of — resolve to
/// [`CapabilityStatus::Unsupported`] rather than reverting, so a newer SDK
/// probing an older deployment fails safe and simply hides the feature.
///
/// Pure read: no storage writes, no events, no authorization, never panics.
pub fn supports_capability(env: &Env, capability: &Symbol) -> CapabilityStatus {
    let caps = get_capabilities(env);

    // ── Cross-cutting ──
    if *capability == Symbol::new(env, "rbac") {
        return caps.rbac;
    }
    if *capability == Symbol::new(env, "two_step_governance") {
        return caps.two_step_governance;
    }
    if *capability == Symbol::new(env, "sep41") {
        return caps.sep41_token_interface;
    }

    // ── Compliance ──
    if *capability == Symbol::new(env, "compliance") {
        return status_of(caps.compliance.module_enabled);
    }
    if *capability == Symbol::new(env, "whitelist") {
        return caps.compliance.whitelist;
    }
    if *capability == Symbol::new(env, "whitelist_revocation") {
        return caps.compliance.whitelist_revocation;
    }
    if *capability == Symbol::new(env, "batch_whitelisting") {
        return caps.compliance.batch_whitelisting;
    }
    if *capability == Symbol::new(env, "compliance_batch_updates") {
        return caps.compliance.batch_status_updates;
    }
    if *capability == Symbol::new(env, "investor_tiers") {
        return caps.compliance.investor_tiers;
    }
    if *capability == Symbol::new(env, "compliance_lifecycle") {
        return caps.compliance.lifecycle_states;
    }
    if *capability == Symbol::new(env, "compliance_transitions") {
        return caps.compliance.lifecycle_transitions;
    }
    if *capability == Symbol::new(env, "eligibility_reads") {
        return caps.compliance.eligibility_reads;
    }

    // ── Minting ──
    if *capability == Symbol::new(env, "minting") {
        return caps.minting.minting;
    }
    if *capability == Symbol::new(env, "burning") {
        return caps.minting.burning;
    }
    if *capability == Symbol::new(env, "supply_cap") {
        return caps.minting.supply_cap;
    }
    if *capability == Symbol::new(env, "yield_distribution") {
        return caps.minting.yield_distribution;
    }

    // ── Transfers ──
    if *capability == Symbol::new(env, "transfers") {
        return caps.transfers.transfers;
    }
    if *capability == Symbol::new(env, "holding_cap") {
        return caps.transfers.holding_cap;
    }
    if *capability == Symbol::new(env, "allowances") {
        return caps.transfers.allowances;
    }
    if *capability == Symbol::new(env, "transfer_from") {
        return caps.transfers.transfer_from;
    }
    if *capability == Symbol::new(env, "transfer_fees") {
        return caps.transfers.transfer_fees;
    }
    if *capability == Symbol::new(env, "transfer_eligibility") {
        return caps.transfers.transfer_eligibility_check;
    }
    if *capability == Symbol::new(env, "transfer_restriction_reasons") {
        return caps.transfers.transfer_restriction_reasons;
    }

    // ── Pause & lifecycle ──
    if *capability == Symbol::new(env, "pause") {
        return caps.pause.global_pause;
    }
    if *capability == Symbol::new(env, "asset_lifecycle") {
        return caps.pause.asset_lifecycle;
    }

    // ── Metadata ──
    if *capability == Symbol::new(env, "metadata") {
        return caps.metadata.name_and_symbol;
    }
    if *capability == Symbol::new(env, "metadata_uri") {
        return caps.metadata.metadata_uri;
    }
    if *capability == Symbol::new(env, "decimals") {
        return caps.metadata.decimals;
    }

    // ── Config ──
    if *capability == Symbol::new(env, "global_config") {
        return caps.config.global_config;
    }

    // ── Events ──
    if *capability == Symbol::new(env, "events") {
        return status_of(caps.events.module_enabled);
    }
    if *capability == Symbol::new(env, "compliance_lifecycle_events") {
        return caps.events.compliance_lifecycle_events;
    }
    if *capability == Symbol::new(env, "transfer_restriction_events") {
        return caps.events.transfer_restriction_events;
    }
    if *capability == Symbol::new(env, "asset_registered_event") {
        return caps.events.asset_registered_event;
    }

    // Unknown / future capability — fail safe.
    CapabilityStatus::Unsupported
}

/// Returns every capability key this contract version understands.
///
/// Lets a client enumerate and cache the registry instead of hardcoding key
/// strings, and lets it detect at runtime that a deployment is older or newer
/// than the keys it knows about. Order is stable within a schema version.
pub fn get_capability_keys(env: &Env) -> Vec<Symbol> {
    vec![
        env,
        Symbol::new(env, "rbac"),
        Symbol::new(env, "two_step_governance"),
        Symbol::new(env, "sep41"),
        Symbol::new(env, "compliance"),
        Symbol::new(env, "whitelist"),
        Symbol::new(env, "whitelist_revocation"),
        Symbol::new(env, "batch_whitelisting"),
        Symbol::new(env, "compliance_batch_updates"),
        Symbol::new(env, "investor_tiers"),
        Symbol::new(env, "compliance_lifecycle"),
        Symbol::new(env, "compliance_transitions"),
        Symbol::new(env, "eligibility_reads"),
        Symbol::new(env, "minting"),
        Symbol::new(env, "burning"),
        Symbol::new(env, "supply_cap"),
        Symbol::new(env, "yield_distribution"),
        Symbol::new(env, "transfers"),
        Symbol::new(env, "holding_cap"),
        Symbol::new(env, "allowances"),
        Symbol::new(env, "transfer_from"),
        Symbol::new(env, "transfer_fees"),
        Symbol::new(env, "transfer_eligibility"),
        Symbol::new(env, "transfer_restriction_reasons"),
        Symbol::new(env, "pause"),
        Symbol::new(env, "asset_lifecycle"),
        Symbol::new(env, "metadata"),
        Symbol::new(env, "metadata_uri"),
        Symbol::new(env, "decimals"),
        Symbol::new(env, "events"),
        Symbol::new(env, "compliance_lifecycle_events"),
        Symbol::new(env, "transfer_restriction_events"),
        Symbol::new(env, "asset_registered_event"),
        Symbol::new(env, "global_config"),
    ]
}

// ─── Interface compatibility checks ────────────────────────────────────────────

/// How a client's known schema version relates to this deployment's.
///
/// Derived purely from comparing two `u32`s against the append-only
/// versioning contract described in `docs/capabilities.md`.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaVersionRelation {
    /// The client was built against exactly this schema version.
    Matching,
    /// The client is older than this deployment: the contract may advertise
    /// fields the client has never heard of. Safe — schema fields are
    /// append-only, so nothing the client already understands has moved.
    ClientOlder,
    /// The client is newer than this deployment: the client may expect
    /// fields or keys this deployment predates. Check `unsupported_required`
    /// rather than assuming the mismatch alone is fatal.
    ClientNewer,
}

/// Result of checking an SDK/dashboard's expected interface against this
/// deployment's actual capability surface.
///
/// See [`check_interface_compatibility`]. This is a diagnostic, not a
/// permission check — like [`ContractCapabilities`], it never gates
/// authorization, only feature availability.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceCompatibilityReport {
    /// This deployment's [`CAPABILITY_SCHEMA_VERSION`].
    pub contract_schema_version: u32,
    /// The schema version the calling client was built against.
    pub client_schema_version: u32,
    /// How the two versions relate.
    pub schema_relation: SchemaVersionRelation,
    /// The subset of `required_capabilities` (from the call) that this
    /// deployment does **not** resolve to `Supported` — including any key
    /// the deployment has never heard of, per the fail-safe rule in
    /// `supports_capability`. Empty means every requirement is met.
    pub unsupported_required: Vec<Symbol>,
    /// `true` iff `unsupported_required` is empty. A schema-version mismatch
    /// alone does not make a client incompatible — only a missing required
    /// capability does, since fields are append-only.
    pub compatible: bool,
}

/// Checks whether a client's required capabilities are all `Supported` by
/// this deployment, and reports how the client's schema version compares.
///
/// `required_capabilities` is the set of capability keys (see
/// `get_capability_keys`) the calling SDK/dashboard build cannot function
/// without. This lets integrators — including RWA/compliance tooling that
/// must not silently degrade — fail fast with a precise, actionable list
/// instead of discovering a gap mid-transaction.
///
/// Pure read: no storage writes, no events, no authorization, never panics.
/// Always available, including before `initialize`.
pub fn check_interface_compatibility(
    env: &Env,
    client_schema_version: u32,
    required_capabilities: &Vec<Symbol>,
) -> InterfaceCompatibilityReport {
    let contract_schema_version = CAPABILITY_SCHEMA_VERSION;

    let schema_relation = if client_schema_version == contract_schema_version {
        SchemaVersionRelation::Matching
    } else if client_schema_version < contract_schema_version {
        SchemaVersionRelation::ClientOlder
    } else {
        SchemaVersionRelation::ClientNewer
    };

    // Re-derive each requirement from the single source of truth so this
    // can never disagree with `supports_capability` / `get_capabilities`.
    let mut unsupported_required: Vec<Symbol> = vec![env];
    for i in 0..required_capabilities.len() {
        let key = required_capabilities.get(i).unwrap();
        if supports_capability(env, &key) != CapabilityStatus::Supported {
            unsupported_required.push_back(key);
        }
    }

    InterfaceCompatibilityReport {
        contract_schema_version,
        client_schema_version,
        schema_relation,
        compatible: unsupported_required.is_empty(),
        unsupported_required,
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

#[contractimpl]
impl AegisContract {
    /// Returns the read-only capability descriptor for this deployment:
    /// which modules are enabled, which protocol behaviours are supported,
    /// planned, or unsupported, and the capability/contract versions.
    ///
    /// Never mutates state, emits no events, requires no authorization, and
    /// remains callable before `initialize` and while paused. See
    /// `docs/capabilities.md`.
    pub fn get_capabilities(env: Env) -> ContractCapabilities {
        get_capabilities(&env)
    }

    /// Returns the [`CapabilityStatus`] for a single capability key.
    ///
    /// Unknown keys resolve to `Unsupported` instead of reverting, so clients
    /// built against a newer contract version fail safe against an older
    /// deployment. Never mutates state; always available. See
    /// `docs/capabilities.md` for the key registry.
    pub fn supports_capability(env: Env, capability: Symbol) -> CapabilityStatus {
        supports_capability(&env, &capability)
    }

    /// Returns every capability key understood by this contract version, for
    /// clients that want to enumerate the registry rather than hardcode it.
    /// Never mutates state; always available.
    pub fn get_capability_keys(env: Env) -> Vec<Symbol> {
        get_capability_keys(&env)
    }

    /// Checks a client's required capability keys against this deployment
    /// and reports the schema-version relationship, for public-interface
    /// compatibility checks ahead of integration. See
    /// `docs/interface-compatibility.md`.
    ///
    /// Never mutates state, emits no events, requires no authorization, and
    /// remains callable before `initialize` and while paused.
    pub fn check_interface_compatibility(
        env: Env,
        client_schema_version: u32,
        required_capabilities: Vec<Symbol>,
    ) -> InterfaceCompatibilityReport {
        check_interface_compatibility(&env, client_schema_version, &required_capabilities)
    }
}
