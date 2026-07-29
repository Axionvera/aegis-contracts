# Audit Evidence Index

*One-stop index of every security-relevant document, contract module, and test
in this repository. Built for external auditors and security reviewers.*

**Repo:** `aegis-contracts` · **Scope:** on-chain contract (`src/`) + read-only
monitoring sidecar (`.github/monitoring/`) · **Maintained:** hand-updated as
evidence lands (see §7)

> **Honesty policy:** something is only listed as ✅ *covered* if the linked
> evidence exists and passes in this tree today. Everything else is ⚠ or ❌ and
> appears again in [§5 Known audit gaps](#5-known-audit-gaps) — that section is
> intentionally uncomfortable reading.

---

## 1. Readiness summary — the 11 issue areas at a glance

| Area | Status | One-line answer | Details |
|---|---|---|---|
| Compliance enforcement | ✅ covered | Whitelist ACL gates mint & transfer on-chain | [§3.1](#31-compliance-enforcement) |
| Admin roles | ⚠ partial | Single admin key, enforced everywhere; no rotation/multisig | [§3.2](#32-admin-roles) |
| Minting | ✅ covered | Admin-only, whitelist-gated, supply-tracked, event-emitted | [§3.3](#33-minting) |
| Transfers | ✅ covered | Holder-authed, two-party whitelist gate, balance check | [§3.4](#34-transfers) |
| Asset metadata | ❌ not implemented | No name/symbol/decimals; not a full SEP-41 token | [§3.5](#35-asset-metadata) |
| Storage | ✅ documented, ⚠ TTL gap | Two-tier layout documented; rent-bump calls absent | [§3.6](#36-storage) |
| Events | ✅ covered | Canonical 5-event `aegis` namespace, XDR-pinned against host | [§3.7](#37-events) |
| Errors | ⚠ partial | 8 distinct string-panics; no stable error codes | [§3.8](#38-errors) |
| Pause | ❌ not implemented | No pause/freeze path exists | [§3.9](#39-pause) |
| Migration | ❌ not implemented | No upgrade hook or versioned storage | [§3.10](#310-migration--upgradeability) |
| Test coverage | ⚠ partial | 9 contract tests + 106 sidecar tests; auth/fuzz gaps remain | [§3.11](#311-test-coverage) |

---

## 2. Primary documentation evidence

| Document | Contents | Relevant to |
|---|---|---|
| [`README.md`](../README.md) | Build, test, monitoring entry points | onboarding, repro |
| [`docs/architecture.md`](architecture.md) | Module separation, **storage tiers**, event layer, monitoring tier | storage, events, threats |
| [`docs/contract-spec.md`](contract-spec.md) | **Public API spec** (5 entry points) + **emitted-event schema** (topics/data table) | API, events, errors |
| [`docs/threat-model.md`](threat-model.md) | Assets, actors, 14 threats w/ mitigations & residual risk | threats, gaps |
| [`CONTRIBUTING.md`](../CONTRIBUTING.md) | Test-mandatory PR policy, fmt/clippy gate | process evidence |
| [`FINDINGS.md`](../FINDINGS.md) | Prior audit-readiness review: event surface added, decoder validated 19/19 + 23/23 against host XDR | history, events |
| [`CHANGED FILES.md`](../CHANGED FILES.md) | Authoritative diff manifest of the prior fix | change control |
| [`.github/monitoring/README.md`](../.github/monitoring/README.md) | Off-chain event streaming/filtering/alerting service config & API | monitoring, replay |
| [`Cargo.toml`](../Cargo.toml) · [`Cargo.lock`](../Cargo.lock) | `soroban-sdk v26`; release profile uses `overflow-checks = true`, `panic = "abort"`; lockfile committed | build security, reprodu. |

---

## 3. Area-by-area evidence

### 3.1 Compliance enforcement

Compliance = *only KYC-whitelisted addresses may hold or move value.*

- **Design docs:** [`docs/architecture.md`](architecture.md)
  ("`compliance.rs` handles all ACL"); [`docs/contract-spec.md`](contract-spec.md)
  per-entry-point revert conditions.
- **Implementation:**
  [`whitelist_user`](../src/compliance.rs#L8-L22) — admin auth → persistent
  `Whitelist(user) = true` → `WhitelistAdd` event.
  [`is_whitelisted`](../src/compliance.rs#L26-L31) — shared helper queried by
  [`mint_asset`](../src/asset.rs#L14-L17) and
  [`transfer`](../src/asset.rs#L45-L53) **before** any state change.
- **Domain event:** `WhitelistAdd` topics `("aegis","wl_add",user)` → `admin`
  ([`events.rs`](../src/events.rs#L43-L51)) — the off-chain compliance-velocity
  alerts key off this.
- **Tests:** `test_whitelist_emits_compliance_event`
  ([`test.rs`](../src/test.rs#L99)),
  `test_mint_to_non_whitelisted_fails` ([`test.rs`](../src/test.rs#L273)).
- **Honest notes:** no de-whitelist/blacklist; entries never expire; no
  negative-transfer-compliance test; batch whitelisting is a TODO
  ([`compliance.rs`](../src/compliance.rs#L18)).

### 3.2 Admin roles

- **Model:** exactly **one** `Admin` address, set once at
  [`initialize`](../src/lib.rs#L26-L35) (instance storage), checked via
  `require_auth` + `assert_eq!` at `whitelist_user`
  ([`compliance.rs`](../src/compliance.rs#L9-L14)),
  `mint_asset` ([`asset.rs`](../src/asset.rs#L9-L11)), and
  `distribute_yield` ([`asset.rs`](../src/asset.rs#L82-L84)).
- **Doc:** [`docs/contract-spec.md`](contract-spec.md) ("Requires admin auth" ×3);
  threat T1/T7 in [`docs/threat-model.md`](threat-model.md).
- **Tests:** privileged paths exercised in `test_lifecycle`
  ([`test.rs`](../src/test.rs#L51)) — **with `mock_all_auths`, so signature
  enforcement is not independently proven** (gap G-5).
- **Honest notes:** no multi-admin, no role rotation, no renounce, no
  timelock; `initialize` itself takes no `require_auth` (deployment-time
  front-run risk, threat T7).

### 3.3 Minting

- **Docs:** [`contract-spec.md`](contract-spec.md) —
  "Mints `amount` to `to`… Reverts if `to` is not whitelisted."
- **Implementation:** [`mint_asset`](../src/asset.rs#L8-L38): admin auth →
  `amount > 0` → whitelisted recipient → persistent balance update → instance
  `TotalSupply` update → `Mint` event carrying `[amount, new_balance,
  total_supply]` for replay-free supply analytics
  ([`events.rs`](../src/events.rs#L55-L64)).
- **Tests:** `test_mint_emits_event_with_balance_and_supply`
  ([`test.rs`](../src/test.rs#L126)) asserts exact running balance/supply in
  the event payload; compliance gate test in §3.1.
- **Honest notes:** unbounded mint by design (RWA supply issuance); i128
  overflow aborts the call (`overflow-checks`); no dedicated overflow or
  zero/negative-amount tests.

### 3.4 Transfers

- **Docs:** [`contract-spec.md`](contract-spec.md) — auth + 3 revert conditions.
- **Implementation:** [`transfer`](../src/asset.rs#L41-L77): `from` auth →
  positive amount → **both** parties whitelisted → balance check → persistent
  debit/credit → `Transfer` event mirroring SEP-41 topic layout
  ([`events.rs`](../src/events.rs#L70-L78)).
- **Tests:** `test_transfer_emits_event_with_both_parties`
  ([`test.rs`](../src/test.rs#L154)),
  `test_transfer_insufficient_balance_fails` ([`test.rs`](../src/test.rs#L287)).
- **Honest notes:** fee deduction is an explicit TODO
  ([`asset.rs`](../src/asset.rs#L61)); negative-compliance cases untested;
  no partial-failure path exists (atomic by construction in Soroban).

### 3.5 Asset metadata

- **Status: ❌ not implemented.** There is no `name`, `symbol`, `decimals`,
  or any contract-level asset descriptor in `src/`, and the contract is **not
  a full SEP-41 token** (no `burn`/`allowance`/`approve`).
- What exists instead: the `Transfer` event's topic layout intentionally
  mirrors SEP-41 ([`events.rs`](../src/events.rs#L66-L69)) so generic Stellar
  tooling can parse movements, and the README describes the asset class
  ("fractional tokenization of Real-World Assets").
- **Action:** tracked as gap G-2; metadata must be added (or SEP-41 adopted)
  before wallets can render the token.

### 3.6 Storage

- **Docs:** [`docs/architecture.md`](architecture.md#ledger-state-storage) —
  "**Instance:** `Admin`, `TotalSupply` … **Persistent:** `Whitelist`, `Balance`
  … must be rent-exempted appropriately."
- **Implementation:** single source of truth is the
  [`DataKey` enum](../src/lib.rs#L13-L18) (`Admin`, `Whitelist(Address)`,
  `Balance(Address)`, `TotalSupply`); all access is instantiate-scoped via
  `env.storage().instance()` for config/supply and
  `env.storage().persistent()` for balances/whitelist.
- **Honest notes (⚠):** the doc promises rent-exemption but **no
  `extend_ttl`/bump call exists** — archival risk tracked as gap G-6 / threat T9.
- Read-only access patterns are exercised end-to-end by the replay/store tests
  in the monitoring tier (see §3.11).

### 3.7 Events

- **Schema docs:** topic layout + data table in
  [`docs/contract-spec.md`](contract-spec.md#emitted-events); design rationale
  in the module doc header of [`events.rs`](../src/events.rs#L1-L31)
  (namespaced topic 0, ≤4 topics, counterparty indexing).
- **Implementation:** 5 `#[contractevent]` types — `Init`, `WhitelistAdd`,
  `Mint`, `Transfer`, `YieldDistributed`
  ([`events.rs`](../src/events.rs)) — published only through
  thin helpers ([`events.rs`](../src/events.rs#L97-L137)); business modules
  never call `env.events()` directly (single source of truth, threat T8).
- **Tests (strongest evidence in the repo):**
  `test_every_state_change_is_observable` ([`test.rs`](../src/test.rs#L209))
  proves the *"one namespaced event per state mutation"* invariant across a
  full lifecycle; per-event shape tests for
  [init](../src/test.rs#L79), [whitelist](../src/test.rs#L99),
  [mint](../src/test.rs#L126), [transfer](../src/test.rs#L154),
  [yield](../src/test.rs#L184); and
  [`.github/monitoring/tests/onchain-compat.test.js`](../.github/monitoring/tests/onchain-compat.test.js)
  pins the off-chain decoder against **host-produced XDR** (regenerate via
  `make dump-events`).

### 3.8 Errors

- **Docs:** per-entry-point revert conditions in
  [`contract-spec.md`](contract-spec.md); full string inventory below.
- **Implementation (⚠ string panics, no `contracterror!` codes):**

  | Where | Condition | Panic string |
  |---|---|---|
  | [`lib.rs`](../src/lib.rs#L27-L30) | re-initialize | `Contract already initialized` |
  | [`compliance.rs`](../src/compliance.rs#L11-L14) | non-admin whitelist | `Unauthorized: Only admin can whitelist` |
  | [`asset.rs`](../src/asset.rs#L11) | non-admin mint | `Unauthorized: Only admin can mint` |
  | [`asset.rs`](../src/asset.rs#L84) | non-admin yield | `Unauthorized` |
  | [`asset.rs`](../src/asset.rs#L12), [L43](../src/asset.rs#L43), [L85](../src/asset.rs#L85) | non-positive amount | `Amount must be positive` |
  | [`asset.rs`](../src/asset.rs#L14-L17) | mint to stranger | `Receiver is not whitelisted` |
  | [`asset.rs`](../src/asset.rs#L45-L53) | transfer w/ stranger | `Sender is not whitelisted` / `Receiver is not whitelisted` |
  | [`asset.rs`](../src/asset.rs#L59) | overdraw | `Insufficient balance` |

- **Tests:** assert-message `should_panic` tests for
  [mint compliance](../src/test.rs#L273) and
  [overdraw](../src/test.rs#L287).
- **Honest notes:** no stable numeric codes (clients must string-match —
  threat T12); calls before `initialize` abort via `unwrap()` on the missing
  `Admin` key rather than a descriptive error (fails closed, ugly message).

### 3.9 Pause

- **Status: ❌ not implemented.** No `pause()` entry point, no paused-state
  flag in `DataKey`, no pause event in [`events.rs`](../src/events.rs), no
  mention in [`contract-spec.md`](contract-spec.md).
- **Impact:** in an incident (key compromise T1, oracle/regulatory freeze
  order) the only current "mitigation" is off-chain alerting from the
  monitoring sidecar — detection, not containment.
- **Action:** gap G-1 — add a two-role pause (pause admin + unpause policy)
  with a `("aegis","pause")` event, or consciously document the trade-off.

### 3.10 Migration / upgradeability

- **Status: ❌ not implemented.** No `upgrade`/migration entry point, no
  WASM-hash rotation, no storage versioning key, no migration tests, and the
  [Makefile](../Makefile) has a build/`optimize` pipeline but no upgrade lane.
- **Impact:** any post-deploy bug fix requires redeploying under a new
  contract ID plus off-chain coordinate migration of clients/monitors.
- **Action:** gap G-3 — decide the upgrade story (upgradeable contract with
  admin-gated `upgrade()`, vs. documented redeploy procedure) before mainnet.

### 3.11 Test coverage

**On-chain (`src/test.rs`, `make test` → 9 passed, 1 fixture helper ignored):**

| # | Test | Covers | Line |
|---|---|---|---|
| 1 | `test_lifecycle` | end-to-end happy path: init→whitelist→mint→transfer | [L51](../src/test.rs#L51) |
| 2 | `test_initialize_emits_event` | Init event shape | [L79](../src/test.rs#L79) |
| 3 | `test_whitelist_emits_compliance_event` | compliance event shape | [L99](../src/test.rs#L99) |
| 4 | `test_mint_emits_event_with_balance_and_supply` | mint accounting payload | [L126](../src/test.rs#L126) |
| 5 | `test_transfer_emits_event_with_both_parties` | transfer event shape | [L154](../src/test.rs#L154) |
| 6 | `test_distribute_yield_emits_event` | yield event payload | [L184](../src/test.rs#L184) |
| 7 | `test_every_state_change_is_observable` | **invariant I8**: 6 mutations → 6 namespaced events | [L209](../src/test.rs#L209) |
| 8 | `test_mint_to_non_whitelisted_fails` | compliance gate on mint | [L273](../src/test.rs#L273) |
| 9 | `test_transfer_insufficient_balance_fails` | overdraw guard | [L287](../src/test.rs#L287) |
| — | `dump_event_xdr` (ignored, `make dump-events`) | regenerates host-XDR fixtures for the monitor seam | [L309](../src/test.rs#L309) |

**Off-chain monitoring (`.github/monitoring/tests/`, `make monitor-test` → 106 tests):**

| File | Tests | Audit relevance |
|---|---:|---|
| [`onchain-compat.test.js`](../.github/monitoring/tests/onchain-compat.test.js) | 10 | contract↔monitor seam: decoder proven against real host XDR |
| [`scval.test.js`](../.github/monitoring/tests/scval.test.js) | 10 | XDR decode exactness (incl. i128, strkey) |
| [`filter.test.js`](../.github/monitoring/tests/filter.test.js) | 14 | compliance event routing correctness |
| [`alert.test.js`](../.github/monitoring/tests/alert.test.js) | 14 | pattern rules (incl. drain detection) |
| [`stream.test.js`](../.github/monitoring/tests/stream.test.js) | 14 | transport resilience (WS↔poll fallback) |
| [`store.test.js`](../.github/monitoring/tests/store.test.js) | 14 | evidence persistence, replay, checkpoints |
| [`triggers.test.js`](../.github/monitoring/tests/triggers.test.js) | 18 | automated reaction guards |
| [`integration.test.js`](../.github/monitoring/tests/integration.test.js) | 12 | end-to-end pipeline + dashboard API |

**Uncovered (honest):** negative-auth tests (all tests run under
`mock_all_auths`), double-initialize, non-positive amounts, non-whitelisted
transfer parties, overflow boundaries, TTL/archival behavior, pause/migration
(feature-absent), fuzz or property tests. See §5.

---

## 4. Security invariant register

| ID | Invariant | Enforced at | Verified by | Status |
|---|---|---|---|---|
| I1 | Initialize at most once | [`lib.rs`](../src/lib.rs#L26-L32) | — | ⚠ enforced, **untested** |
| I2 | Admin-only whitelist | [`compliance.rs`](../src/compliance.rs#L9-L14) | happy-path only (`mock_all_auths`) | ⚠ enforced, negative untested |
| I3 | Admin-only mint | [`asset.rs`](../src/asset.rs#L9-L11) | happy-path only | ⚠ enforced, negative untested |
| I4 | Mint only to whitelisted | [`asset.rs`](../src/asset.rs#L14-L17) | `test_mint_to_non_whitelisted_fails` | ✅ tested |
| I5 | Both transfer parties whitelisted | [`asset.rs`](../src/asset.rs#L45-L53) | happy-path only | ⚠ enforced, negative untested |
| I6 | No overdraw | [`asset.rs`](../src/asset.rs#L59) | `test_transfer_insufficient_balance_fails` | ✅ tested |
| I7 | Positive amounts (mint/transfer/yield) | [`asset.rs`](../src/asset.rs#L12), [L43](../src/asset.rs#L43), [L85](../src/asset.rs#L85) | — | ⚠ enforced, **untested** |
| I8 | 1 namespaced event per mutation | [`events.rs`](../src/events.rs) helpers-only publishing | `test_every_state_change_is_observable` + `onchain-compat.test.js` | ✅ tested |
| I9 | `TotalSupply == Σ balances` | construction (mint-only supply, no burn, no fee yet) | implied by mint accounting test | ⚠ holds; property test absent |

---

## 5. Known audit gaps

*Ordered by audit impact. None are hidden — this list is the todo list for
reaching audit-ready rather than audit-indexed.*

| ID | Gap | Impact | Suggested next step |
|---|---|---|---|
| **G-1** | **No pause / emergency stop** (§3.9) | HIGH — incident response impossible on-chain | two-role pause + `pause`/`unpause` events |
| **G-2** | **No asset metadata / not SEP-41** (§3.5) | MEDIUM — wallets/explorers can't render; unclear token semantics | add `name/symbol/decimals`, or adopt SEP-41 interface |
| **G-3** | **No migration/upgrade path** (§3.10) | MEDIUM — fixes require redeploy + off-chain coordination | choose upgradeable-contract vs. documented redeploy plan |
| **G-4** | Single-admin trust model, no rotation (§3.2, T1) | HIGH residual risk | multisig admin / smart-account admin, rotation + timelock |
| **G-5** | **All tests mock auth** (`mock_all_auths`); no negative-auth or signature tests (T14) | MEDIUM — auth enforcement unproven | add `mock_auths`-scoped negative tests for I2/I3 |
| **G-6** | **No TTL/rent bumping** on persistent state (§3.6, T9) | MEDIUM — balances may archive | `extend_ttl` on whitelist/balance writes + monitor alert |
| **G-7** | String-only errors, no `contracterror!` codes (§3.8, T12) | LOW | numeric error enum; update spec |
| **G-8** | No de-whitelist/blacklist/clawback; whitelist entries are permanent | MEDIUM for regulated assets | removal + freeze events, tested |
| **G-9** | `initialize` takes no `require_auth` (T7); init-before-deploy race | LOW-MEDIUM | `admin.require_auth()`; document atomic deploy |
| **G-10** | Stubbed features by design: `distribute_yield` only emits an event ([`asset.rs`](../src/asset.rs#L81-L96)); fee deduction TODO ([`asset.rs`](../src/asset.rs#L61)); batch whitelist TODO ([`compliance.rs`](../src/compliance.rs#L18)) | functional | implement behind spec + tests before mainnet |
| **G-11** | No fuzz/property tests, no formal verification, **no external audit has been performed** | process | proptest for I6/I9, symbolic checks, commission audit |
| **G-12** | Toolchain unpinned (no `rust-toolchain.toml`); `wasm32v1-none` choice lives only in the [Makefile](../Makefile) | reproducibility | pin toolchain + SDK version policy |
| **G-13** | ~~Monitoring tree hygiene~~ — **fixed while building this index:** `src/analytics/Index.js` case-mismatch broke 3 test files on case-sensitive FS; `store.tests.js` was never discovered by `node --test` (14 tests silently skipped); `README`/`Makefile` pointed at `monitoring/` while the service lives at `.github/monitoring/` | would have hidden evidence | resolved — see [`CHANGED FILES.md`](../CHANGED FILES.md) / commit for the renames |
| **G-14** | `distribute_yield`'s mock semantics could be mistaken for real payouts by consumers of the `yield` event | LOW transparency risk | rename event or emit explicit `simulated` flag once real distribution lands |

---

## 6. Reproduce every claim

```bash
make build          # wasm32v1-none release build (Rust ≥1.84, soroban-sdk v26)
make test           # on-chain suite: expect 9 passed, 1 ignored
make dump-events    # regenerate host-XDR fixtures (ignored test)
make test-all       # on-chain + monitoring suites: expect 9 + 106 passing
cargo clippy --all-targets && cargo fmt --all --check   # lint/format gates
```

Environment used when this index was authored: Rust stable (via rustup),
`wasm32v1-none`, Node ≥18 for the monitoring tier. `Cargo.lock` is committed;
SDK is pinned to `soroban-sdk = "26.0.0"`.

## 7. Maintaining this index

- Any PR touching `src/**` or `docs/**` must update the evidence rows +
  invariant register above (rule proposed alongside
  [`CONTRIBUTING.md`](../CONTRIBUTING.md)'s test requirement).
- Gaps close top-down: G-1..G-4 first (safety), then G-5/G-6 (assurance),
  then polish.
