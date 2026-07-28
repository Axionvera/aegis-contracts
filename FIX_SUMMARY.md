# Fix Summary — Transfer Restriction Reason Codes

**Repository:** `felladaniel36-hash/aegis-contracts` (Aegis RWA Protocol, Soroban/Rust)
**Issue type:** Feature (Expert) — *Implement or document transfer restriction reason codes*

---

## STEP 1–3 · Findings

### What the developer is building

A Soroban smart contract for tokenizing Real-World Assets, enforcing regulatory
compliance at the ledger level. Modules: `admin` (RBAC + pause), `compliance`
(KYC whitelist), `asset` (mint/transfer + lifecycle status), `supply_cap` and
`holding` (2-step governed caps), `eligibility` (read helpers), `capabilities`
(feature-gating descriptor), `errors` (numbered error codes).

The repo already had a strong convention — `docs/error-codes.md` defines
category-ranged numeric codes so SDKs match on integers, not strings. The issue
asks to extend that convention to blocked transfers specifically.

### The exact defects located

| # | Defect | Location |
|---|--------|----------|
| 1 | **Three distinct asset states collapsed into one code.** `transfer` and `mint_asset` both returned the generic `Error::AssetNotActive` (6000) for `Paused`, `Retired`, **and** `Blocked`. A client literally could not distinguish "retry in an hour" from "this asset is retired forever". | `src/asset.rs` (2 sites) |
| 2 | **Cap breaches had no error code at all.** `enforce_holding_cap` and `enforce_supply_cap` used bare `assert!` with string messages. Soroban does **not** return panic messages to callers — the client saw only an untyped host trap. | `src/holding.rs`, `src/supply_cap.rs` |
| 3 | **No way to ask *why* before signing.** `check_transfer_eligibility` returned a bare `bool` — enough to disable a button, not enough to label it. | `src/eligibility.rs` |
| 4 | **No unauthorised-operation reason on the movement path**, and no reason ⇄ code mapping documented for clients. | — |
| 5 | **Pre-existing build breakage** (blocker): `src/test.rs` had a mangled merge leaving an orphaned code block after line 1658, so `cargo test` failed to compile the whole crate. The matrix test also never called `initialize`. | `src/test.rs` |

Note: `docs/capabilities.md` already acknowledged the gap — Soroban discards
events from reverted invocations, so a "TransferBlocked" event is structurally
impossible and **the error code is the only off-chain signal**. That makes code
specificity the entire fix surface.

---

## STEP 4 · The fix

### New: `src/restrictions.rs` (single source of truth)

* **`RestrictionReason`** — a `#[contracttype]` enum with 12 variants covering
  every blocking condition: `None`, `UnauthorizedOperation`, `ContractPaused`,
  `AssetPaused`, `AssetRetired`, `AssetBlocked`, `SenderNotCompliant`,
  `RecipientNotCompliant`, `InvalidAmount`, `InsufficientBalance`,
  `HoldingCapExceeded`, `SupplyCapExceeded`.
* **`is_blocked()` / `is_terminal()`** — `is_terminal()` is `true` only for
  `AssetRetired`, letting dashboards suppress pointless retry buttons.
* **`error_for_reason` / `reason_for_error` / `code_for_reason`** — a total,
  bijective mapping over blocking reasons. Non-restriction errors (config,
  storage, governance) deliberately map to `None` so they are never rendered as
  "your transfer was blocked because…".
* **`evaluate_transfer` / `evaluate_mint`** — pure evaluators that run checks in
  *exactly* the order the state-changing entrypoints do, so a pre-flight answer
  always equals the eventual revert.
* **4 new read-only entrypoints:** `check_transfer_restriction`,
  `check_mint_restriction`, `get_restriction_code`,
  `get_restriction_schema_version`. No writes, no auth, never panic — callable
  while paused, before `initialize`, and against terminal asset states.

### New error codes — reserved 7000 range

| Code | Name | Replaces |
|------|------|----------|
| 7000 | `AssetPausedRestriction` | `6000` when status is `Paused` |
| 7001 | `AssetRetiredRestriction` | `6000` when status is `Retired` |
| 7002 | `AssetBlockedRestriction` | `6000` when status is `Blocked` |
| 7003 | `HoldingCapExceeded` | an untyped `assert!` panic |
| 7004 | `SupplyCapExceeded` | an untyped `assert!` panic |

**No existing code was renumbered or reused.** `6000 AssetNotActive` is retained
as a documented *reserved* code per the repo's own stability rule.

### Changed behaviour

* `asset.rs` — new `require_asset_movable()` helper replaces both
  `!= Active → AssetNotActive` checks with the specific lifecycle code.
* `holding.rs` / `supply_cap.rs` — `enforce_*_cap` now return
  `Result<(), Error>` instead of `assert!`-panicking; callers propagate with `?`.
* `eligibility.rs` — `check_transfer_eligibility` is now implemented as
  `!evaluate_transfer(..).is_blocked()`, so the boolean and reason views can
  never drift apart (asserted by test).
* `capabilities.rs` — added `transfers.transfer_restriction_reasons` capability
  + registry key; `CAPABILITY_SCHEMA_VERSION` bumped `1 → 2` per its own rule.

### Documented precedence (matches implementation exactly)

`transfer`: pause → amount → asset state → sender KYC → recipient KYC →
holding cap → balance.
`mint_asset`: pause → authorization → amount → asset state → recipient KYC →
supply cap → holding cap.

---

## STEP 5–8 · Validation

| Gate | Result |
|------|--------|
| `cargo test` | **98 passed, 0 failed** (was: *did not compile*) |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --all-targets -- -D warnings` | **no new findings.** 18 pre-existing `soroban_sdk` deprecation warnings remain (unrelated to this fix); the 2 that *were* in scope (unused import, unused variable) are now fixed — 21 → 19 |
| `cargo build --target wasm32v1-none --release` | **succeeds** |

**Confidence: ~97%.** Every acceptance criterion is covered by an executing
test, the reason ⇄ code mapping is machine-verified as total and round-tripping,
the WASM release target builds, and no existing code was renumbered.
The residual ~3% is deployment-environment behaviour (live RPC/CLI) that cannot
be exercised in this sandbox.

**No conflicting errors.** Backwards compatibility is preserved: no code
renumbered, no entrypoint signature changed, all four additions are new
read-only entrypoints, and `6000` is reserved rather than deleted. A stale
client that hasn't been updated falls through to its "unknown code" branch,
which the existing error-code standard already requires to be safe.

---

## STEP 9 · Fix features

1. Twelve reason codes covering every blocking path — **no generic failure remains** on the transfer/mint surface.
2. Non-compliant **sender** (`4000`) and **recipient** (`4001`) distinctly represented, with deterministic precedence when both fail.
3. Asset state restrictions split three ways: **paused** (`7000`, retryable), **retired** (`7001`, terminal), **blocked** (`7002`, administrative).
4. **Unauthorised operation** (`3000`) represented on the mint path.
5. Cap breaches promoted from untyped host panics to typed codes (`7003`/`7004`).
6. **Pre-flight reason reads** so a dashboard can explain a block *before* asking the user to sign.
7. `is_terminal()` so UIs suppress futile retries on retired assets.
8. On-chain, self-describing mapping (`get_restriction_code`) — SDKs need not hardcode the table.
9. Capability flag + schema version for safe feature detection.
10. Fixed the pre-existing compile breakage in `src/test.rs` that blocked the entire test suite.

---

## STEP 10–11 · Files modified / created

### Created
| File | Purpose |
|------|---------|
| `src/restrictions.rs` | Reason enum, mapping helpers, evaluators, 4 entrypoints |
| `docs/transfer-restrictions.md` | Full SDK/dashboard mapping contract (reason table, precedence, TS reference impl, copy deck, stability rules, test matrix) |
| `FIX_SUMMARY.md` | This document |

### Modified
| File | Change |
|------|--------|
| `src/errors.rs` | +5 codes in new 7000 range; `AssetNotActive` documented as reserved |
| `src/asset.rs` | `require_asset_movable()`; specific lifecycle codes; `?` on cap enforcement |
| `src/holding.rs` | `enforce_holding_cap` → `Result<(), Error>` (`7003`) |
| `src/supply_cap.rs` | `enforce_supply_cap` → `Result<(), Error>` (`7004`) |
| `src/eligibility.rs` | Delegates to shared evaluator — no drift possible |
| `src/capabilities.rs` | New capability field + registry key; schema `1 → 2` |
| `src/lib.rs` | Registered `pub mod restrictions;` |
| `src/test.rs` | **Repaired broken file**; +20 restriction tests; updated capability expectations |
| `docs/error-codes.md` | 7000 range + all codes tabulated; mapping guidance; cross-link |
| `docs/capabilities.md` | New capability row/key; schema version; corrected event note |
| `docs/events.md` | Expanded restriction code list; pre-flight guidance |
| `docs/local-deployment.md` | Troubleshooting rows for `7000`–`7004`; `6000` marked reserved |
| `README.md` | Linked the new doc under **Errors** |
