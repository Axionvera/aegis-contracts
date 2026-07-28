use soroban_sdk::contracterror;

/// Standardized contract error codes for the Aegis RWA Protocol.
///
/// Codes are grouped into stable, non-overlapping ranges by category so
/// downstream SDKs and dashboards can classify a failure from its numeric
/// code alone, without depending on human-readable revert strings. See
/// `docs/error-codes.md` for the full SDK/dashboard mapping guidance.
///
/// Ranges are deliberately spaced 1000 apart so new variants can be added
/// to a category without ever renumbering another category.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    // ─── 1000s: Configuration ──────────────────────────────────────────────
    /// The contract has already been initialized and cannot be reconfigured.
    AlreadyInitialized = 1000,

    // ─── 2000s: Storage ─────────────────────────────────────────────────────
    /// No admin is set in storage; the contract has not been initialized.
    NotInitialized = 2000,
    /// There is no pending admin transfer recorded in storage to accept.
    NoPendingAdminTransfer = 2001,

    // ─── 3000s: Admin & Authorization ──────────────────────────────────────
    /// Caller does not hold the role or admin rights required for this call.
    Unauthorized = 3000,
    /// The Admin role cannot be assigned via `set_role`; use `transfer_admin`.
    CannotAssignAdminRole = 3001,
    /// The target address has no role assigned, so there is nothing to revoke.
    NoRoleToRevoke = 3002,
    /// Caller is not the address recorded as the pending admin candidate.
    NotPendingCandidate = 3003,
    /// The contract is paused; this state-changing operation is blocked.
    ContractPaused = 3004,
    /// The contract is already paused.
    AlreadyPaused = 3005,
    /// The contract is not currently paused.
    NotPaused = 3006,

    // ─── 4000s: Compliance ──────────────────────────────────────────────────
    /// The sending address is not on the compliance whitelist.
    SenderNotWhitelisted = 4000,
    /// The receiving address is not on the compliance whitelist.
    ReceiverNotWhitelisted = 4001,

    // ─── 5000s: Minting & Transfers ─────────────────────────────────────────
    /// The requested amount must be strictly greater than zero.
    InvalidAmount = 5000,
    /// The sender's balance is insufficient to cover the requested amount.
    InsufficientBalance = 5001,
    // ─── 6000s: Asset Metadata ──────────────────────────────────────────────
    /// Metadata update is blocked in the current lifecycle status
    /// (asset is Retired or Blocked).
    AssetMetadataUpdateBlocked = 6002,

    // ─── 7000s: Asset Lifecycle ─────────────────────────────────────────────
    /// The asset is in Draft status; minting and transfers are not permitted
    /// until the asset is activated.
    AssetNotActive = 7000,
    /// The asset is in Paused (lifecycle) status; minting and transfers are
    /// suspended until the asset is reactivated.
    AssetLifecyclePaused = 7001,
    /// The asset is Retired; all minting and transfers are permanently blocked.
    AssetRetired = 7002,
    /// The asset is Blocked; minting and transfers are suspended pending
    /// regulatory or admin review.
    AssetBlocked = 7003,
    /// The requested lifecycle transition is not valid from the current status.
    InvalidLifecycleTransition = 7004,
}
