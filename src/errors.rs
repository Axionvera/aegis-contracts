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
    /// Minting would exceed the active global supply cap.
    SupplyCapExceeded = 5002,
    /// The investor's balance would exceed the active holding cap.
    HoldingCapExceeded = 5003,
    // ─── 6000s: Asset Metadata ──────────────────────────────────────────────
    // Reserved for future asset-metadata validation errors (name, symbol,
    // decimals, schema checks). No active failure paths use this range yet.
    /// Minting/transfers are restricted because the asset is not Active.
    AssetNotActive = 6000,
    /// Asset status transition is not allowed by lifecycle rules.
    InvalidAssetStatusTransition = 6001,
    /// Metadata update is blocked in the current lifecycle status.
    AssetMetadataUpdateBlocked = 6002,
}
