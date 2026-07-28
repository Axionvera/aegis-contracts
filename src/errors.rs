use soroban_sdk::contracterror;

/// Standardized contract error codes for the Aegis RWA Protocol.
///
/// Codes are grouped into stable, non-overlapping ranges by category so
/// downstream SDKs and dashboards can classify a failure from its numeric code
/// alone. Codes are append-only: never reuse or renumber an existing value.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    // 1000s: Configuration
    /// The contract has already been initialized and cannot be reconfigured.
    AlreadyInitialized = 1000,

    // 2000s: Storage
    /// No admin is set in storage; the contract has not been initialized.
    NotInitialized = 2000,
    /// There is no pending admin transfer recorded in storage to accept.
    NoPendingAdminTransfer = 2001,

    // 3000s: Admin & Authorization
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

    // 4000s: Compliance
    /// The sending address has no current clearance (`Unknown` or `Revoked`).
    SenderNotWhitelisted = 4000,
    /// The receiving address has no current clearance (`Unknown` or `Revoked`).
    ReceiverNotWhitelisted = 4001,
    /// The sending address is `Blocked`.
    SenderBlocked = 4002,
    /// The receiving address is `Blocked`.
    ReceiverBlocked = 4003,
    /// The sending address is `Pending`.
    SenderCompliancePending = 4004,
    /// The receiving address is `Pending`.
    ReceiverCompliancePending = 4005,
    /// The requested compliance transition is not permitted.
    InvalidComplianceTransition = 4006,
    /// The requested compliance status equals the current status.
    ComplianceStatusUnchanged = 4007,

    // 5000s: Minting & Transfers
    /// The requested amount must be strictly greater than zero.
    InvalidAmount = 5000,
    /// The sender's balance is insufficient to cover the requested amount.
    InsufficientBalance = 5001,
    /// Minting would exceed the active global supply cap.
    SupplyCapExceeded = 5002,
    /// The investor's balance would exceed the active holding cap.
    HoldingCapExceeded = 5003,

    // 6000s: Asset Lifecycle & Metadata
    /// The asset is in Draft status.
    AssetNotActive = 6000,
    /// The asset is in Paused lifecycle status.
    AssetLifecyclePaused = 6001,
    /// The asset is Retired.
    AssetRetired = 6002,
    /// The asset is Blocked.
    AssetBlocked = 6003,
    /// The requested lifecycle transition is not valid from the current status.
    InvalidLifecycleTransition = 6004,
    /// Asset status transition is not allowed by lifecycle rules.
    InvalidAssetStatusTransition = 6005,
    /// Metadata update is blocked in the current lifecycle status.
    AssetMetadataUpdateBlocked = 6006,

    // 7000s: Transfer / Movement Restrictions
    /// The asset lifecycle status is `Paused`.
    AssetPausedRestriction = 7000,
    /// The asset lifecycle status is `Retired`.
    AssetRetiredRestriction = 7001,
    /// The asset lifecycle status is `Blocked`.
    AssetBlockedRestriction = 7002,
}
