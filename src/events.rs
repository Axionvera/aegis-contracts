//! Canonical on-chain event definitions for the Aegis RWA Protocol.
//!
//! Every state mutation in the protocol publishes a structured contract event so
//! that the off-chain monitoring service (`/monitoring`) can stream, filter,
//! route, alert on, persist and replay protocol activity in real time.
//!
//! # Topic layout
//!
//! All Aegis events share a stable, greppable shape:
//!
//! ```text
//! topics = ("aegis", <action>, [indexed subject...])
//! data   = <action specific payload>
//! ```
//!
//! Anchoring topic 0 to the `aegis` namespace lets an off-chain consumer
//! subscribe to the entire protocol with a single Soroban RPC topic filter
//! (`["AAAADwAAAAVhZWdpcwAAAA==", "*"]`) while still being able to narrow down
//! to one action by pinning topic 1. Addresses are indexed as topics so the RPC
//! itself can filter by counterparty.
//!
//! Topic counts stay at or below the Soroban limit of four topics per event.
//!
//! Events are declared with `#[contractevent]`, which generates the topic/data
//! encoding, the XDR schema entry in the contract spec, and a `publish` method.

use soroban_sdk::{contractevent, Address, Env};

/// `("aegis", "init")` -> `{ admin }`
///
/// Signals that the contract instance is live and under the control of `admin`.
/// Monitoring uses this as the anchor ledger for a deployment.
#[contractevent(topics = ["aegis", "init"], data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Init {
    pub admin: Address,
}

/// `("aegis", "wl_add", user)` -> `admin`
///
/// Compliance-critical: records which admin granted whitelist access to which
/// address. Drives the compliance-velocity alert rules off-chain.
#[contractevent(topics = ["aegis", "wl_add"], data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhitelistAdd {
    #[topic]
    pub user: Address,
    pub admin: Address,
}

/// `("aegis", "wl_rev", user)` -> `admin`
///
/// Compliance-critical: records revocation/suspension of a previously whitelisted
/// investor. When this event is observed, off-chain systems must:
/// - Mark user as non-compliant and frozen
/// - Stop allowing future mints/transfers to this address
/// - Alert risk desk for potential forced redemption / off-boarding
/// Policy: revoked users cannot receive new restricted tokens (mint/transfer-in)
/// and cannot send (transfer-out) - fully frozen, but retain historical balance.
#[contractevent(topics = ["aegis", "wl_rev"], data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhitelistRevoked {
    #[topic]
    pub user: Address,
    pub admin: Address,
}

/// `("aegis", "mint", to)` -> `[amount, new_balance, total_supply]`
///
/// Publishing the resulting balance and supply alongside the delta lets the
/// analytics dashboard chart supply growth without replaying the whole ledger.
#[contractevent(topics = ["aegis", "mint"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mint {
    #[topic]
    pub to: Address,
    pub amount: i128,
    pub new_balance: i128,
    pub total_supply: i128,
}

/// `("aegis", "transfer", from, to)` -> `amount`
///
/// Mirrors the SEP-41 style `transfer` topic layout so generic Stellar tooling
/// can consume Aegis transfers, while the `aegis` namespace keeps them
/// distinguishable from classic token events.
#[contractevent(topics = ["aegis", "transfer"], data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transfer {
    #[topic]
    pub from: Address,
    #[topic]
    pub to: Address,
    pub amount: i128,
}

/// `("aegis", "yield")` -> `[admin, amount, total_supply]`
///
/// The contract spec documents `distribute_yield` as "triggers a dividend yield
/// event for off-chain indexing" - this is that event.
#[contractevent(topics = ["aegis", "yield"], data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YieldDistributed {
    pub admin: Address,
    pub amount: i128,
    pub total_supply: i128,
}

// ---------------------------------------------------------------- Helpers
//
// Thin wrappers keep call sites in the business-logic modules readable and give
// us a single place to evolve the event surface.

pub fn contract_initialized(env: &Env, admin: &Address) {
    Init {
        admin: admin.clone(),
    }
    .publish(env);
}

pub fn user_whitelisted(env: &Env, admin: &Address, user: &Address) {
    WhitelistAdd {
        user: user.clone(),
        admin: admin.clone(),
    }
    .publish(env);
}

pub fn user_revoked(env: &Env, admin: &Address, user: &Address) {
    WhitelistRevoked {
        user: user.clone(),
        admin: admin.clone(),
    }
    .publish(env);
}

pub fn asset_minted(env: &Env, to: &Address, amount: i128, new_balance: i128, total_supply: i128) {
    Mint {
        to: to.clone(),
        amount,
        new_balance,
        total_supply,
    }
    .publish(env);
}

pub fn asset_transferred(env: &Env, from: &Address, to: &Address, amount: i128) {
    Transfer {
        from: from.clone(),
        to: to.clone(),
        amount,
    }
    .publish(env);
}

pub fn yield_distributed(env: &Env, admin: &Address, amount: i128, total_supply: i128) {
    YieldDistributed {
        admin: admin.clone(),
        amount,
        total_supply,
    }
    .publish(env);
}
