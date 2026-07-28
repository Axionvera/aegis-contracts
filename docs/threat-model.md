# Aegis RWA Protocol — Threat Model

This document is the top-level security threat model for the Aegis RWA
Protocol smart contracts. It identifies the assets the contracts protect, the
trust boundaries and actors in the system, the assumptions the contracts rely
on, the threats considered across the compliance, admin, issuance, and
transfer surfaces, and the residual risks that remain after existing
mitigations. It also states explicitly what is **out of scope** — smart
contract logic is one control in a much larger compliance and legal system,
and this document draws that line deliberately.

This is a living document. Category-specific detail already documented
elsewhere in `docs/` is cross-referenced rather than duplicated; this document
is the map that ties those pieces together into a single threat picture.

> **This document does not replace legal, regulatory, or compliance-operations
> review.** The Aegis contracts enforce a technical whitelist and a set of
> on-chain invariants. They do not determine legal eligibility, perform KYC/AML
> screening, verify sanctions status, hold custody of the underlying
> real-world asset, or constitute a legal opinion on the token's regulatory
> treatment. See [Out of scope](#5-out-of-scope) below.

## 1. Protected assets

| Asset | On-chain representation | Why it matters |
|---|---|---|
| Investor token balances | `DataKey::Balance(Address)` | Represents an investor's claim on the tokenized RWA; unauthorized change is direct financial harm. |
| Total supply integrity | `DataKey::TotalSupply` | Backs the claim that issuance is bounded and auditable; uncontrolled inflation devalues all holders. |
| Compliance whitelist | `DataKey::Whitelist(Address)` | The sole on-chain gate enforcing that only eligible investors can hold or move tokens. |
| Role assignments & admin identity | `DataKey::Role(Address)`, `DataKey::Admin`, `DataKey::AdminCandidate` | Determines who can mint, whitelist, pause, or reconfigure the protocol. |
| Supply cap / holding cap governance state | `DataKey::SupplyCap`, `DataKey::HoldingCap` (+ `*Candidate`) | Investor-protection and issuance ceilings; integrity of the 2-step amendment flow. |
| Pause state | `DataKey::Paused` | The protocol's kill switch; both its correct operation and its non-abuse are assets. |
| Event stream | All `env.events().publish(...)` calls (see [`docs/events.md`](events.md)) | The only off-chain-observable audit trail; indexers, dashboards, and compliance tooling depend on its completeness and stability. |

## 2. Actors and trust boundaries

| Actor | Trust level | On-chain capability |
|---|---|---|
| **Admin** (`DataKey::Admin`) | Fully trusted, highest privilege | Bypasses all role checks (`require_role`/`require_any_role`); can mint, whitelist, pause/unpause, assign/revoke roles, transfer or renounce admin, and govern supply/holding caps. |
| **ComplianceOfficer** | Trusted, scoped | `whitelist_user`, `revoke_whitelist`. |
| **AssetManager** | Trusted, scoped | `mint_asset`, `distribute_yield`. |
| **EmergencyOfficer** | Trusted, **combined** scope | Everything ComplianceOfficer and AssetManager can do, **plus** `pause` (see [§4.3](#43-role-misuse-and-privilege-blast-radius)). |
| **Investor / token holder** | Semi-trusted, self-scoped | Can call `transfer` for their own address (`from.require_auth()`); no privileged calls. |
| **Off-chain KYC/AML/compliance function** | External, trusted input source | Not a contract actor — but every whitelist decision the contract enforces originates entirely from this off-chain process. See [§4.8](#48-off-chain-compliance-assumptions). |
| **Off-chain indexers, SDKs, dashboards** | External consumer | Read contract state and events; cannot influence contract state. Correctness of their output depends on the assumptions in [§4.7](#47-event-reliability-and-downstream-indexing). |
| **Unauthenticated public** | Untrusted | Can call any read function (`get_balance_of`, `get_total_supply`, `is_whitelisted`, `is_paused`, `get_role_of`, `get_investor_eligibility`, `check_transfer_eligibility`, etc.) with no authorization. |

**Trust boundary:** the line between "on-chain enforceable" and "off-chain
attested" runs directly through the whitelist. Everything on the contract side
of that line (whitelist checks, role checks, caps, pause) is enforced by
Soroban's execution and auth model. Everything on the other side (who
*should* be whitelisted, whether an asset's real-world backing exists, whether
a jurisdiction permits the offering) is an off-chain trust assumption the
contract cannot verify.

## 3. Assumptions

- The Stellar network and Soroban host environment execute contract logic
  correctly and enforce `require_auth()` as documented; consensus- and
  VM-level security are out of scope for this document.
- `overflow-checks = true` is set in `[profile.release]` ([`Cargo.toml`](../Cargo.toml)),
  so `i128` arithmetic overflow reverts the transaction instead of wrapping.
  This document assumes release builds are always deployed with this flag
  intact.
- The Aegis contract models **one token per deployment** ([`docs/architecture.md`](architecture.md));
  supply cap and holding cap are both single global values. Multi-asset
  issuance requires separate contract instances, each with its own trust
  assumptions repeated independently.
- A whitelist entry (`Whitelist(Address) == true`) is treated by the contract
  as a permanent "eligible" flag until explicitly revoked. The contract has no
  concept of expiry, re-verification, jurisdiction, or accreditation tier —
  see [§4.1](#41-compliance-bypass).
- Private keys for Admin, ComplianceOfficer, AssetManager, and EmergencyOfficer
  roles are assumed to be held and secured off-chain (hardware wallet,
  multisig, or equivalent), per the recommendations in
  [`docs/admin-misuse-risks.md`](admin-misuse-risks.md). The contract has no
  on-chain second factor for any privileged call.
- `distribute_yield` is documented in-code as a **mock implementation**
  ("Mock implementation... TODO: Implement scalable yield snapshot
  mechanism") — it emits an event but moves no balances. Any off-chain
  system must not treat `yield_distributed` as proof that value was
  transferred.

## 4. Threat catalog

Each threat below states the scenario, the existing mitigation (with the
enforcing code path), and the residual risk that remains.

### 4.1 Compliance bypass

| # | Scenario | Mitigation | Residual risk |
|---|---|---|---|
| C-1 | Mint or transfer to a non-whitelisted address. | `mint_asset`/`transfer` check `compliance::is_whitelisted` for every party and revert with `ReceiverNotWhitelisted` (4001) / `SenderNotWhitelisted` (4000). | None for direct calls — enforced on every state-changing path, including admin-initiated mints. |
| C-2 | A whitelist entry goes stale: an investor's real-world eligibility lapses (sanctions designation, jurisdiction change, offering closes, subscription terminated) but `Whitelist(Address)` is never revoked on-chain. | None on-chain — `Whitelist(Address)` has no expiry or re-verification. This is an off-chain compliance monitoring responsibility (`revoke_whitelist`). | **High.** The contract has no mechanism to detect or react to a real-world eligibility change; it can only enforce what it has been told. This is the single largest gap between "on-chain compliant" and "actually compliant" — see [§4.8](#48-off-chain-compliance-assumptions). |
| C-3 | A compromised or malicious ComplianceOfficer whitelists an ineligible address, and it receives a mint or transfer before the whitelisting is caught and reversed. | Scoped role, revocation capability, event emission (`user_whitelisted`) for off-chain alerting — see [`docs/admin-misuse-risks.md` §3](admin-misuse-risks.md#3-whitelist-abuse). | **Medium.** A race exists between the malicious whitelist call and off-chain detection; funds already moved are not automatically clawed back. |
| C-4 | `Whitelist(Address)` is a single boolean (the code itself labels it "Legacy whitelist flag ... kept for backwards compatibility"). It carries no jurisdiction, accreditation tier, or investor-class data, so it cannot express regime-specific rules (e.g. Reg D vs. Reg S investor separation, per-country transfer restrictions). | Out of scope for the current contract version. | **By design, until a richer eligibility model ships.** Deployers relying on multi-jurisdiction offerings must implement that segmentation entirely off-chain today. |

### 4.2 Admin key compromise

Fully catalogued in [`docs/admin-misuse-risks.md` §1](admin-misuse-risks.md#1-single-point-of-failure-admin-key-compromise)
and [`docs/emergency-pause.md`](emergency-pause.md#trust-model). Summary for
this model:

- A compromised Admin key grants **every** privileged capability at once:
  mint (bypasses `AssetManager` check), whitelist (bypasses
  `ComplianceOfficer` check), pause/unpause, role assignment/removal, and
  admin transfer.
- The 2-step `transfer_admin`/`accept_admin` flow protects against
  *misdirecting* an admin transfer (wrong address, typo, premature effect).
  It does **not** protect against key theft — an attacker holding the current
  admin key can complete both steps as themselves, or simply act directly
  without ever transferring.
- **Residual risk: Critical / Low likelihood** given standard hardware-wallet
  or multisig custody, as recommended. The contract has no on-chain
  mitigation beyond scoped roles and event-based alerting; custody security is
  entirely an off-chain operational control.

### 4.3 Role misuse and privilege blast radius

- Per-role misuse scenarios (malicious `AssetManager` minting to an
  accomplice, malicious `ComplianceOfficer` whitelist abuse, stale role
  assignments, role-stacking design) are catalogued in
  [`docs/admin-misuse-risks.md` §2, §3, §5, §6](admin-misuse-risks.md).
- **Threat model note**: `EmergencyOfficer`
  combines `ComplianceOfficer` **and** `AssetManager` privileges in a single
  role, plus `pause`. A compromised `EmergencyOfficer` key can mint to a
  whitelisted accomplice *and* whitelist that accomplice itself *and* pause
  the contract to cover its tracks or block a response — a strictly larger
  blast radius than any other non-admin role. It is missing only role
  management, admin transfer, and `unpause` relative to full Admin
  compromise. Deployers should treat `EmergencyOfficer` key custody with
  Admin-equivalent rigor, not merely "elevated operator" rigor, and should
  avoid assigning it unless both compliance and asset privileges are
  genuinely needed on the same operational key
  ([`docs/admin-roles.md`](admin-roles.md)).
- `require_role`/`require_any_role` implement the *only* authorization
  surface; there is no time-lock, rate-limit, or multi-party approval on any
  single privileged call — a single compromised scoped key is immediately
  and fully effective.

### 4.4 Minting / asset issuance abuse

| # | Scenario | Mitigation | Residual risk |
|---|---|---|---|
| M-1 | Unauthorized (non-`AssetManager`/`Admin`) mint. | `require_role(&env, &admin, Role::AssetManager)` in `mint_asset`. | None for external callers. |
| M-2 | Mint to a non-whitelisted address. | `ReceiverNotWhitelisted` (4001) check before crediting. | None. |
| M-3 | Unbounded inflation. | `enforce_supply_cap` runs before every mint, for every caller including Admin/AssetManager, per [`docs/supply-cap-governance.md`](supply-cap-governance.md). | **The supply cap defaults to `0` ("no cap enforced") until the admin explicitly proposes and accepts one.** A deployment that never configures a cap has **no on-chain ceiling on total issuance** beyond whitelist gating. This is a configuration risk, not a code defect — deployers must treat setting a supply cap as a required go-live step, not an optional hardening measure. |
| M-4 | Zero/negative-amount mint. | `InvalidAmount` (5000) if `amount <= 0`. | None. |
| M-5 | A malicious `AssetManager` mints legitimate-looking amounts to a whitelisted accomplice. | Scoped role, off-chain supply/event auditing — see [`docs/admin-misuse-risks.md` §2`](admin-misuse-risks.md#2-unauthorized-minting). | Residual, inherent to any privileged-mint design; mitigated only by careful role assignment. |
| M-6 | `distribute_yield` is invoked and its `yield_distributed` event is treated by an off-chain system as evidence that yield was actually paid. | None on-chain — the function is an explicitly documented mock (see [§3](#3-assumptions)) that moves no balances. | **High for any integration that has not read the source.** Any dashboard, accounting system, or investor-facing statement that derives payout amounts from this event alone will misstate reality until a real yield-settlement mechanism ships. Integrators must treat `yield_distributed` as a notification of *intent*, not settlement, until this is resolved. |

### 4.5 Transfer restriction failure

| # | Scenario | Mitigation | Residual risk |
|---|---|---|---|
| T-1 | Transfer from or to a non-whitelisted address. | `SenderNotWhitelisted` (4000) / `ReceiverNotWhitelisted` (4001) checked before any balance mutation. | None. |
| T-2 | Transfer pushes the receiver's balance above the per-investor holding cap. | `holding::enforce_holding_cap` runs before crediting the receiver in both `mint_asset` and `transfer`, per [`docs/investor-holding-restrictions.md`](investor-holding-restrictions.md). | None for the enforcement itself. Note the cap is intentionally **not** retroactive: lowering the cap below an existing holder's balance does not claw back funds — documented, expected behavior, not a bypass. |
| T-3 | Transfer while the contract is paused. | `require_not_paused` at the top of `transfer`. | None. |
| T-4 | Insufficient sender balance. | `InsufficientBalance` (5001) check before debiting. | None. |
| T-5 | No on-chain transaction-velocity or maximum-transfer-size control (e.g. structuring/AML transaction-monitoring patterns). | Not implemented — and not a Soroban-contract-layer concern by design. | **By design.** Real-time transaction monitoring for AML purposes is assumed to be an off-chain function layered on top of the event stream, not an on-chain control. |
| T-6 | `transfer` carries a `// TODO: Implement fee deduction on transfer` comment — the transfer path is not yet feature-complete relative to its eventual design. | N/A — not yet implemented. | **Forward-looking risk, not a present bypass.** When a fee mechanism is added, it must be re-reviewed against compliance checks (e.g., a fee recipient must also satisfy whitelist rules) so this threat model should be revisited at that time. |

### 4.6 Pause mechanism misuse

Fully catalogued in [`docs/emergency-pause.md`](emergency-pause.md) and
[`docs/admin-misuse-risks.md` §7, §8](admin-misuse-risks.md#7-emergency-pause-misuse).
Summary for this model:

- Pause is a single **global** switch (no per-function granularity, a
  deliberate design choice per [`docs/emergency-pause.md`](emergency-pause.md#comparison-with-other-pause-designs)).
  Any future finer-grained pause control changes this threat surface and
  should be reflected here when it ships.
- `EmergencyOfficer` (or Admin) can pause; only Admin can unpause. This
  asymmetry is intentional: it bounds a compromised `EmergencyOfficer` to a
  denial-of-service risk, not a fund-theft risk, but it also means an
  unavailable or hostile Admin after a pause leaves the protocol frozen with
  no on-chain recovery path.
- **Residual risk: Medium (DoS via compromised EmergencyOfficer), High
  (indefinite freeze if Admin is unavailable or hostile after a pause)** —
  both are pre-existing, documented, accepted trade-offs of the current
  design, not defects.

### 4.7 Event reliability and downstream indexing

Full schema and compatibility guarantees are in [`docs/events.md`](events.md).
Threat-model-relevant points:

- **Reverted calls emit nothing.** Soroban discards all events from an
  invocation that ultimately reverts, so a compliance-blocked mint/transfer
  attempt produces **no on-chain event** — only the standardized revert code
  (`4000`, `4001`, `3004`, ...) is observable, and only to a caller who
  simulates or submits the transaction. A monitoring system that watches
  *only* the emitted-event stream for "attempted violations" will see
  nothing for blocked attempts; it must additionally monitor failed
  transaction submissions/simulations at the RPC or indexer layer.
- **No off-chain data in events, by design** ([`docs/events.md`](events.md#conventions)):
  every event carries only addresses, amounts, and role enum values.
  Correlating an on-chain event with the off-chain KYC identity behind an
  address requires a separate, trusted address-to-identity mapping
  maintained entirely off-chain. The integrity of *that* mapping is outside
  the contract's ability to verify or protect, and is a single point of
  failure for any investigation or audit that needs to go from "which
  address" to "which investor."
- **Schema stability is a documented convention, not an on-chain guarantee.**
  SDKs are told to match on topic string and field name, never struct
  position ([`docs/events.md`](events.md#why)); a consumer that ignores this
  guidance and decodes positionally could silently misinterpret a payload
  after a future contract upgrade. This is a downstream integration risk, not
  a contract defect.

### 4.8 Off-chain compliance assumptions

This is the section every reader of this threat model should treat as load
-bearing: **the contract enforces a boolean flag; it does not perform
compliance.**

- The contract has no knowledge of, and cannot verify: KYC outcome, AML
  screening results, sanctions-list membership, accreditation status,
  jurisdiction, subscription agreement terms, or offering-document
  restrictions. All of these must be evaluated by an off-chain compliance
  function whose *conclusion* is expressed on-chain solely as
  `whitelist_user` / `revoke_whitelist` calls.
- The contract has no oracle or proof-of-reserve mechanism linking
  `TotalSupply` to the actual existence, custody, or legal title of the
  underlying real-world asset. `TotalSupply` is a ledger counter of tokens
  issued, not a verified claim that real-world backing exists in matching
  quantity. Custody of the underlying asset is a legal and operational
  arrangement entirely outside this repository's scope.
- The legal enforceability of a whitelist decision, a pause action, or a
  transfer restriction rests on the underlying legal agreements (subscription
  agreements, offering memoranda, applicable securities law) — not on the
  Soroban transaction that encoded it. **A correctly functioning smart
  contract is a technical control that supports a compliance program; it is
  not a substitute for one.**
- Whitelist decisions are only as good as the off-chain process that drives
  them. A `ComplianceOfficer` key holder is trusted to whitelist only
  entities that have actually cleared KYC/AML/accreditation checks; the
  contract has no way to verify that a whitelisting was preceded by a real
  compliance check versus none at all.

## 5. Out of scope

The following are explicitly **not** addressed by these contracts or this
threat model, and must be handled by the asset issuer's legal, compliance,
and operational functions:

- Correctness, availability, or security of any off-chain KYC/AML/sanctions
  screening provider.
- Legal validity, securities-law classification, or regulatory enforceability
  of the tokenized instrument in any jurisdiction.
- Custody, safekeeping, insurance, or legal title of the underlying
  real-world asset backing the token.
- Integrity of the off-chain mapping between an on-chain address and a
  real-world investor identity.
- Regulatory reporting obligations (tax withholding, transaction reporting,
  sanctions reporting, etc.).
- Private key generation, storage, and operational custody practices for
  Admin/ComplianceOfficer/AssetManager/EmergencyOfficer keys (policy
  recommendations only, in [`docs/admin-misuse-risks.md`](admin-misuse-risks.md)).
- Stellar network/consensus/validator security and Soroban host VM security.
- Front-end, wallet, or dashboard application security for any client
  integrating with these contracts.
- Network-layer denial-of-service (RPC availability, Stellar network
  congestion) — distinct from the on-chain pause mechanism covered in [§4.6](#46-pause-mechanism-misuse).

## 6. Residual risk summary

| Category | Worst-case residual risk | Primary control |
|---|---|---|
| Compliance bypass (stale whitelist) | High | Off-chain compliance monitoring (contract has no expiry mechanism) |
| Admin key compromise | Critical (low likelihood with proper custody) | Off-chain key custody (hardware wallet / multisig) |
| Role misuse (EmergencyOfficer) | High (combined mint+whitelist+pause blast radius) | Careful role assignment; treat as Admin-equivalent custody |
| Unconfigured supply cap | Medium–High (unbounded issuance by default) | Deployment checklist: set a supply cap before go-live |
| Yield distribution mock | High for uninformed integrators | Documentation; must not be treated as settlement until implemented |
| Transfer restriction failure | Low | Whitelist + holding cap + pause enforced on every state-changing path |
| Pause misuse | Medium (DoS) / High (indefinite freeze) | Asymmetric pause/unpause design; off-chain governance for admin unavailability |
| Event reliability | Medium | Documented event/error-code conventions; monitors must also watch failed transactions |
| Off-chain compliance assumptions | Structural — cannot be reduced to Low by contract code | Legal/compliance program external to this repository |

## 7. Related documents

- [`docs/admin-roles.md`](admin-roles.md) — RBAC design and privileged operation table.
- [`docs/admin-misuse-risks.md`](admin-misuse-risks.md) — detailed admin/role-specific risk catalogue.
- [`docs/emergency-pause.md`](emergency-pause.md) — pause mechanism scope, authorization, and trust model.
- [`docs/supply-cap-governance.md`](supply-cap-governance.md) / [`docs/investor-holding-restrictions.md`](investor-holding-restrictions.md) — 2-step cap governance and enforcement.
- [`docs/events.md`](events.md) — event schema, reliability guarantees, and scope notes.
- [`docs/error-codes.md`](error-codes.md) — standardized revert codes referenced throughout this document.
- [`docs/investor-eligibility.md`](investor-eligibility.md) — read-only eligibility helpers and their point-in-time caveats.
- [`docs/architecture.md`](architecture.md) — module boundaries and storage layout.

## 8. Maintaining this document

Update this threat model whenever: a new privileged role or capability is
added, the pause scope changes, a new compliance control (e.g. jurisdiction
tagging, expiring whitelist entries) ships, `distribute_yield` gains a real
settlement mechanism, or a new asset-metadata module (the reserved `6000`
error range) is introduced. Treat a threat-model update as part of the
acceptance criteria for any PR that changes trust boundaries, not an
afterthought.
