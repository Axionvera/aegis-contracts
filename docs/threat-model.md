# Aegis Contracts — Threat Model

*Status: draft for external review · covers the on-chain contract (`src/`) at
`main` and the read-only monitoring sidecar (`.github/monitoring/`)*

This is the protocol's first-pass threat model. It is **honest about what is
not yet mitigated** — open items are tracked as gaps in
[`docs/audit-evidence-index.md`](./audit-evidence-index.md) §5. Auditors should
treat `⚠ residual` rows below as active risk, not resolved risk.

---

## 1. Scope & assets at risk

| Asset | Why it matters | Storage |
|---|---|---|
| Token balances (RWA fractional ownership) | Financial value; regulated ownership | `DataKey::Balance` — persistent ([`src/lib.rs`](../src/lib.rs#L13-L18)) |
| Total supply integrity | Mint accounting; must equal Σ balances | `DataKey::TotalSupply` — instance |
| Compliance whitelist | Regulatory boundary (KYC gating) | `DataKey::Whitelist` — persistent |
| Admin key | Total control of minting & whitelist | `DataKey::Admin` — instance |
| Event stream | Off-chain compliance/alerting reads it | Emitted per mutation ([`src/events.rs`](../src/events.rs)) |

## 2. Actors & trust assumptions

| Actor | Trust level | Capabilities today |
|---|---|---|
| Admin | **Fully trusted (single key)** | Whitelist, mint, yield-event emission |
| Whitelisted user | Untrusted | Transfers to other whitelisted users |
| Non-whitelisted user | Untrusted | None (cannot hold or receive) |
| Off-chain monitor | Untrusted, **read-only** (holds no keys; never submits transactions) | Alerts, analytics, triggers |
| External auditor | Untrusted | Reads this repo |

Trust assumption carried by the protocol today: **the single admin key is an
EOA-equivalent point of total control.** There is no multisig, rotation,
renunciation, or timelock on-chain (see §4 T1).

## 3. Security objectives (invariants)

Every objective cross-references the invariant register in the audit index
(§4), where enforcement points and verifying tests are linked.

- **O1** Initialization happens at most once.
- **O2** Only the admin may whitelist.
- **O3** Only the admin may mint.
- **O4** Tokens are minted only to whitelisted addresses.
- **O5** Transfers require *both* parties to be whitelisted.
- **O6** No balance may be overdrawn.
- **O7** Mint/transfer/yield amounts must be positive.
- **O8** Every state mutation publishes exactly one `aegis`-namespaced event
  (off-chain compliance depends on this).
- **O9** Supply conservation: `TotalSupply == Σ balances` (no burn exists;
  holds by construction).

## 4. Threats, mitigations, residual risk

| # | Threat | Mitigation in code | Test evidence | Residual |
|---|---|---|---|---|
| **T1** | **Admin key compromise / malicious admin** — attacker mints unbounded supply or whitelists attacker addresses | `require_auth` + admin equality check at every privileged call ([`asset.rs`](../src/asset.rs#L9-L11), [`asset.rs`](../src/asset.rs#L82-L84), [`compliance.rs`](../src/compliance.rs#L9-L14)) | none — tests use `mock_all_auths()` | **⚠ HIGH** — single-key trust; no rotation/multisig/timelock. Off-chain `instant-drain`-style alert rules partially detect abuse (`.github/monitoring/src/defaults.js`) but cannot prevent it |
| **T2** | Unauthorized mint by non-admin | Admin `require_auth` + `assert_eq!` ([`asset.rs`](../src/asset.rs#L11)) | no negative test | ⚠ LOW (auth enforced) but untested with real signatures |
| **T3** | Mint to non-whitelisted address | `assert!(is_whitelisted(...))` ([`asset.rs`](../src/asset.rs#L14-L17)) | `test_mint_to_non_whitelisted_fails` ([`test.rs`](../src/test.rs#L273)) | LOW |
| **T4** | Transfer from/to non-whitelisted address | Two-sided whitelist asserts ([`asset.rs`](../src/asset.rs#L45-L53)) | no negative test | LOW — enforced, untested |
| **T5** | Overdraw / insufficient balance | `assert!(from_balance >= amount)` ([`asset.rs`](../src/asset.rs#L59)) | `test_transfer_insufficient_balance_fails` ([`test.rs`](../src/test.rs#L287)) | LOW |
| **T6** | Arithmetic overflow/underflow (mint, transfer, supply) | `i128` domain + `overflow-checks = true` (release profile, [`Cargo.toml`](../Cargo.toml)); non-positive amounts rejected | no boundary tests | LOW — overflows abort the call rather than corrupt state |
| **T7** | **Initialization front-run** — a third party calls `initialize` before the deployer | `initialize` runs at most once (assert in [`lib.rs`](../src/lib.rs#L26-L32)), *but takes no `require_auth` on the admin parameter* | no double-init test | ⚠ MEDIUM — deployment must pair contract creation with `initialize` atomically; add `admin.require_auth()` as defence-in-depth |
| **T8** | **Event drift** — an accidental topic/shape change silently breaks off-chain compliance alerts & analytics | Single source of truth in [`events.rs`](../src/events.rs); modules never call `env.events()` directly | `test_every_state_change_is_observable` ([`test.rs`](../src/test.rs#L209)) + `onchain-compat.test.js` pins host-produced XDR | LOW (guarded by tests) |
| **T9** | **Persistent-state archival (TTL/rent expiry)** — `Whitelist`/`Balance` entries are persistent but never bumped | `docs/architecture.md` documents the requirement ("rent-exempted appropriately"); **no `extend_ttl`/bump calls exist in code** | none | ⚠ MEDIUM — balances could become unavailable until restored; needs TTL-bump instrumentation and monitoring |
| **T10** | DoS via unbounded iteration (yield, batch ops) | No on-chain iteration exists: `distribute_yield` only emits an event ([`asset.rs`](../src/asset.rs#L81-L96)); batch whitelist is an explicit TODO | `test_distribute_yield_emits_event` | LOW by construction — the TODO list (fee deduction, batch whitelist, yield snapshots) must preserve this |
| **T11** | Reentrancy | Contract performs **no cross-contract calls**; Soroban's shared-storage model doesn't permit mid-call re-entry into the same instance here | n/a | LOW by construction — revisit if token hooks/SEP-41 interop are added |
| **T12** | Error-spoofing / fragile client error handling | n/a — **string panics only; no `contracterror!` enum** | assert-message tests | ⚠ LOW — clients key on human strings, not stable codes (see gaps) |
| **T13** | Monitor-sidecar compromise or unavailability | Sidecar is read-only (no keys, never submits transactions — `docs/architecture.md`); polling fallback if WS RPC degrades | stream/store/trigger tests | LOW — worst case is loss of alerting, not loss of funds |
| **T14** | Test-environment auth masking — `mock_all_auths()` hides signature-enforcement bugs | n/a | all 9 contract tests | ⚠ MEDIUM — no negative-auth coverage; add signature-failure tests before mainnet |

## 5. Out of scope / assumed

- Stellar core and Soroban host correctness (trusted platform).
- Soroban RPC endpoint honesty/availability (monitoring degrades to polling;
  `docs/architecture.md`).
- Legal/regulatory KYC process feeding the whitelist (off-chain by design).
- Off-chain key custody for the admin key.

## 6. Method & limits of this analysis

Manual review of `src/` (303 lines of Rust) plus the 9-test suite and
monitoring test corpora; anchored to the previous audit-readiness review in
[`FINDINGS.md`](../FINDINGS.md). **No fuzzing, formal verification, or third-party
audit has been performed** — see the gap register in
[`docs/audit-evidence-index.md`](./audit-evidence-index.md) §5.
