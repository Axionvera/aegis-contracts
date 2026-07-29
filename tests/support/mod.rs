//! Shared support code for the SDK integration fixture harness.
//!
//! This module is intentionally dependency-free: it hand-rolls a tiny,
//! deterministic JSON writer rather than pulling in `serde_json`, so the
//! fixture harness adds **zero** new entries to `Cargo.toml` / `Cargo.lock`
//! and cannot break a downstream build or a vendored/offline CI run.
//!
//! Everything here is `#[allow(dead_code)]`-tolerant: it is test-only code
//! compiled into `tests/sdk_fixtures.rs`.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

use soroban_sdk::testutils::Events as _;
use soroban_sdk::xdr::{
    ContractEvent, ContractEventBody, ContractEventType, Int128Parts, Limits, ScAddress, ScError,
    ScVal, UInt128Parts, WriteXdr,
};
use soroban_sdk::{Address, Env, IntoVal, TryFromVal, Val};

use aegis_contracts::{AegisContract, AegisContractClient};

// ─── Deterministic synthetic identities ───────────────────────────────────────

/// Documented public derivation seed for every address in the fixture set.
///
/// Each actor's Ed25519 public key is literally `SHA-256("aegis-fixture/<label>")`,
/// rendered as a Stellar `G...` strkey. These are **not** keypairs: no private
/// key exists (or can exist) for them, they hold no funds on any network, and
/// they correspond to no real person or account. See `docs/sdk-fixtures.md`.
pub const ADDRESS_DERIVATION_SEED: &str = "aegis-fixture/<label>";

/// The contract's own address is derived the same way, as a `C...` strkey from
/// `SHA-256("aegis-fixture/contract")`.
pub const CONTRACT_ADDRESS: &str = "CCEOFPHM2IOUTJS53R74QWIEQXXEHLYOTZYMCS44UI735A4WCJZAQNWP";

/// Stable label → strkey table for every actor used by the fixtures.
///
/// Order is meaningful and must stay stable: fixture consumers may index into
/// `actors.json` by position as well as by label.
pub const ACTORS: &[(&str, &str)] = &[
    (
        "admin",
        "GDAVU6P2QJK4IWQWUNYUXBAFGTPF36MGBN5HBZGYCVCKO2DONWP7YDIJ",
    ),
    (
        "compliance_officer",
        "GAEGCFR5CC2J5E5FVFDOJJS4TGNCBWTDMNILRHETSHDWXOXIOFWA25JU",
    ),
    (
        "asset_manager",
        "GBL2V6RSU2K6U3C73HDQOCVWBCQ7H7ZJY6RECEHIUHSZ7M6736CDPBMO",
    ),
    (
        "emergency_officer",
        "GBLPHF3QVD22643G67KSIPOHI2OY5UDQJAW4XKNOZABV2HDXITYQ34FV",
    ),
    (
        "investor_alice",
        "GAXRVA67D5NLMKP6H5IROF3IY5EMQW6AQBJLTUITGAAZUXMGY7CYO2KG",
    ),
    (
        "investor_bob",
        "GD4YM2BO77TT5BMWZC7SMH74GWZ5TPTKVTGPW5X6VUKFXLLD6W3XPRYN",
    ),
    (
        "investor_carol",
        "GCETMP6KNA4OB27H554CJJURJ7W2FCWJZUG3PCMC2DM73FVIARPHCMH2",
    ),
    (
        "outsider_dave",
        "GDEEAPLXZWKS57XMNUD4D4AEWEGKUZUKTRJRCCVA7JEAL6YREHN4BR2F",
    ),
];

/// Looks up an actor strkey by label. Panics on an unknown label so a typo
/// fails loudly at test time instead of silently producing a new address.
pub fn strkey(label: &str) -> &'static str {
    ACTORS
        .iter()
        .find(|(l, _)| *l == label)
        .map(|(_, k)| *k)
        .unwrap_or_else(|| panic!("unknown fixture actor label: {label}"))
}

/// Reverse lookup: strkey → label, used to annotate rendered addresses.
pub fn label_for(addr: &str) -> Option<&'static str> {
    if addr == CONTRACT_ADDRESS {
        return Some("contract");
    }
    ACTORS.iter().find(|(_, k)| *k == addr).map(|(l, _)| *l)
}

// ─── Test environment ─────────────────────────────────────────────────────────

/// A fixture scenario's environment: a fresh `Env` with the contract
/// registered at the fixed [`CONTRACT_ADDRESS`], all auths mocked, and a
/// typed client.
pub struct Harness {
    pub env: Env,
    pub contract_id: Address,
}

impl Harness {
    /// Builds a fresh, fully deterministic harness.
    ///
    /// Registering at a *fixed* contract address (rather than letting the
    /// host generate one) is what makes the emitted fixtures byte-stable
    /// across runs and machines.
    pub fn new() -> Harness {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = Address::from_str(&env, CONTRACT_ADDRESS);
        env.register_at(&contract_id, AegisContract, ());
        Harness { env, contract_id }
    }

    pub fn client(&self) -> AegisContractClient<'_> {
        AegisContractClient::new(&self.env, &self.contract_id)
    }

    /// Resolves a fixture actor label to a live `Address` in this env.
    pub fn actor(&self, label: &str) -> Address {
        Address::from_str(&self.env, strkey(label))
    }

    /// Converts any contract value into its wire-level `ScVal`, then into the
    /// fixture JSON representation. This is what keeps fixtures honest: the
    /// rendered value is derived from the same XDR an SDK sees over the wire,
    /// never hand-written.
    pub fn render<T: IntoVal<Env, Val>>(&self, value: T) -> Json {
        let val: Val = value.into_val(&self.env);
        let sc: ScVal =
            ScVal::try_from_val(&self.env, &val).expect("contract value must convert to ScVal");
        scval_to_json(&sc)
    }

    /// Captures the events published by the **most recent** contract
    /// invocation, rendered for fixtures.
    ///
    /// Note: `env.events().all()` in the Soroban test host returns only the
    /// last invocation's events, so this must be called immediately after the
    /// call whose events you want to record.
    pub fn events(&self) -> Json {
        let all = self.env.events().all();
        let mut out = Vec::new();
        for ev in all.events() {
            out.push(contract_event_to_json(ev));
        }
        Json::Arr(out)
    }

    /// Asserts the exact typed event sequence, then renders the same live
    /// events for the committed fixture.
    ///
    /// Keeping this assertion in the fixture-generation path matters:
    /// `UPDATE_FIXTURES=1` must not be able to bless an accidental topic,
    /// payload, caller, or ordering change merely by rewriting the JSON.
    pub fn assert_events(
        &self,
        expected: soroban_sdk::Vec<(Address, soroban_sdk::Vec<Val>, Val)>,
    ) -> Json {
        assert_eq!(
            self.env.events().all(),
            expected,
            "live event sequence does not match the exported typed fixture expectation"
        );
        self.events()
    }

    /// Asserts that the most recent invocation emitted no durable event, then
    /// renders the empty sequence for a negative-path fixture.
    pub fn assert_no_events(&self) -> Json {
        let actual = self.env.events().all();
        assert_eq!(
            actual.events().len(),
            0,
            "reverted or read-only fixture invocation unexpectedly emitted an event"
        );
        self.events()
    }
}

// ─── ScVal → fixture JSON ─────────────────────────────────────────────────────

/// Renders a single `ContractEvent` (topics + data + raw XDR) for fixtures.
pub fn contract_event_to_json(ev: &ContractEvent) -> Json {
    let mut obj = JsonObj::new();

    let contract = ev
        .contract_id
        .as_ref()
        .map(|cid| ScAddress::Contract(cid.clone()).to_string());
    match &contract {
        Some(c) => {
            obj.push("contract", Json::Str(c.clone()));
        }
        None => {
            obj.push("contract", Json::Null);
        }
    }

    obj.push(
        "type",
        Json::Str(
            match ev.type_ {
                ContractEventType::System => "system",
                ContractEventType::Contract => "contract",
                ContractEventType::Diagnostic => "diagnostic",
            }
            .to_string(),
        ),
    );

    let ContractEventBody::V0(body) = &ev.body;

    // The topic tuple. Aegis publishes a single string topic per event; it is
    // rendered both as the raw array and as a convenience scalar so SDK tests
    // can match on `topic` directly.
    let topics: Vec<Json> = body.topics.iter().map(scval_to_json).collect();
    if let (1, Some(Json::Str(first))) = (topics.len(), topics.first()) {
        obj.push("topic", Json::Str(first.clone()));
    } else {
        obj.push("topic", Json::Null);
    }
    obj.push("topics", Json::Arr(topics));
    obj.push("data", scval_to_json(&body.data));
    obj.push(
        "xdr_base64",
        Json::Str(
            ev.to_xdr_base64(Limits::none())
                .expect("event must serialize to XDR"),
        ),
    );

    obj.build()
}

/// Converts a wire `ScVal` into the fixture JSON representation.
///
/// Representation rules (documented in `docs/sdk-fixtures.md`):
/// * `Address` → Stellar strkey string.
/// * `i128`/`u128`/`i64`/`u64` → **decimal string**, so JSON consumers with
///   IEEE-754 numbers (JavaScript) cannot silently lose precision.
/// * `u32`/`i32` → JSON number (always exactly representable).
/// * `Symbol`/`String` → JSON string.
/// * `Bytes` → lowercase hex string.
/// * `Map` → JSON object keyed by the symbol/string key.
/// * `Vec` → JSON array. Note that a `#[contracttype]` unit enum (e.g. `Role`,
///   `AssetStatus`) is encoded on the wire as a one-element vec containing the
///   variant symbol, so it renders as e.g. `["Admin"]`.
/// * `Void` → `null` (this is how `Option::None` arrives).
/// * `Error` → object with the error `type` and numeric `code`.
pub fn scval_to_json(v: &ScVal) -> Json {
    match v {
        ScVal::Bool(b) => Json::Bool(*b),
        ScVal::Void => Json::Null,
        ScVal::Error(e) => scerror_to_json(e),
        ScVal::U32(u) => Json::Num(*u as i128),
        ScVal::I32(i) => Json::Num(*i as i128),
        ScVal::U64(u) => Json::Str(u.to_string()),
        ScVal::I64(i) => Json::Str(i.to_string()),
        ScVal::Timepoint(t) => Json::Str(t.0.to_string()),
        ScVal::Duration(d) => Json::Str(d.0.to_string()),
        ScVal::U128(p) => Json::Str(u128_from_parts(p).to_string()),
        ScVal::I128(p) => Json::Str(i128_from_parts(p).to_string()),
        ScVal::U256(_) | ScVal::I256(_) => Json::Str(format!("{v:?}")),
        ScVal::Bytes(b) => Json::Str(hex(b.as_slice())),
        ScVal::String(s) => Json::Str(s.to_utf8_string_lossy()),
        ScVal::Symbol(s) => Json::Str(s.to_utf8_string_lossy()),
        ScVal::Vec(Some(items)) => Json::Arr(items.iter().map(scval_to_json).collect()),
        ScVal::Vec(None) => Json::Null,
        ScVal::Map(Some(entries)) => {
            let mut obj = JsonObj::new();
            for e in entries.iter() {
                let key = match scval_to_json(&e.key) {
                    Json::Str(s) => s,
                    other => other.to_compact(),
                };
                obj.push_owned(key, scval_to_json(&e.val));
            }
            obj.build()
        }
        ScVal::Map(None) => Json::Null,
        ScVal::Address(a) => Json::Str(a.to_string()),
        other => Json::Str(format!("{other:?}")),
    }
}

fn scerror_to_json(e: &ScError) -> Json {
    let mut obj = JsonObj::new();
    match e {
        ScError::Contract(code) => {
            obj.push("type", Json::Str("contract".into()));
            obj.push("code", Json::Num(*code as i128));
        }
        other => {
            obj.push("type", Json::Str("host".into()));
            obj.push("detail", Json::Str(format!("{other:?}")));
        }
    }
    obj.build()
}

pub fn i128_from_parts(p: &Int128Parts) -> i128 {
    ((p.hi as i128) << 64) | (p.lo as i128)
}

pub fn u128_from_parts(p: &UInt128Parts) -> u128 {
    ((p.hi as u128) << 64) | (p.lo as u128)
}

pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

// ─── Minimal deterministic JSON ───────────────────────────────────────────────

/// A minimal JSON value used to emit fixtures.
///
/// `Obj` preserves **insertion order** (rather than sorting) so fixtures read
/// in a logical order; because the harness always inserts in the same order,
/// output stays byte-for-byte stable across runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Json {
    Null,
    Bool(bool),
    /// Integers that are always safe to represent as a JSON number.
    Num(i128),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn str(s: impl Into<String>) -> Json {
        Json::Str(s.into())
    }

    /// Renders a compact single-line form (used for map keys).
    pub fn to_compact(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0, false);
        out
    }

    /// Renders the canonical pretty form used for every fixture file: two
    /// space indentation, `\n` line endings, and a trailing newline.
    pub fn to_pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0, true);
        out.push('\n');
        out
    }

    fn write(&self, out: &mut String, indent: usize, pretty: bool) {
        let pad = |out: &mut String, n: usize| {
            if pretty {
                out.push('\n');
                for _ in 0..n {
                    out.push_str("  ");
                }
            }
        };
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Num(n) => {
                let _ = write!(out, "{n}");
            }
            Json::Str(s) => write_json_string(out, s),
            Json::Arr(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    pad(out, indent + 1);
                    item.write(out, indent + 1, pretty);
                }
                pad(out, indent);
                out.push(']');
            }
            Json::Obj(entries) => {
                if entries.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push('{');
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    pad(out, indent + 1);
                    write_json_string(out, k);
                    out.push(':');
                    if pretty {
                        out.push(' ');
                    }
                    v.write(out, indent + 1, pretty);
                }
                pad(out, indent);
                out.push('}');
            }
        }
    }
}

fn write_json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Insertion-ordered JSON object builder.
pub struct JsonObj(Vec<(String, Json)>);

impl JsonObj {
    pub fn new() -> JsonObj {
        JsonObj(Vec::new())
    }

    pub fn push(&mut self, key: &str, value: Json) -> &mut Self {
        self.0.push((key.to_string(), value));
        self
    }

    pub fn push_owned(&mut self, key: String, value: Json) -> &mut Self {
        self.0.push((key, value));
        self
    }

    pub fn build(self) -> Json {
        Json::Obj(self.0)
    }
}

impl Default for JsonObj {
    fn default() -> Self {
        JsonObj::new()
    }
}

// ─── Fixture file IO ──────────────────────────────────────────────────────────

/// Directory (relative to the crate root) holding the committed fixtures.
pub const FIXTURE_DIR: &str = "fixtures/sdk";

pub fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_DIR)
        .join(name)
}

/// Returns true when the harness was asked to rewrite the committed fixtures
/// (`UPDATE_FIXTURES=1 cargo test`).
pub fn update_mode() -> bool {
    matches!(
        std::env::var("UPDATE_FIXTURES").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

/// Writes `value` to `fixtures/sdk/<name>` in update mode, or asserts that the
/// committed file already matches it byte-for-byte after normalizing checkout
/// line endings to the canonical `\n` representation.
///
/// This is the mechanism that makes the fixtures *self-verifying*: if contract
/// behaviour drifts (an event field is renamed, an error code changes, a
/// balance calculation changes) the committed fixture no longer matches the
/// freshly generated one and the test fails, instead of shipping a stale
/// fixture to downstream SDK repos.
pub fn write_or_verify(name: &str, value: &Json) {
    let path = fixture_path(name);
    let rendered = value.to_pretty();

    if update_mode() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("fixture dir must be creatable");
        }
        std::fs::write(&path, rendered.as_bytes()).expect("fixture must be writable");
        return;
    }

    let existing = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing fixture {}: {e}\n\
             Run `UPDATE_FIXTURES=1 cargo test --test sdk_fixtures` to generate it.",
            path.display()
        )
    });

    // Git may materialize a text fixture with CRLF on Windows even though the
    // committed blob and deterministic renderer use LF. Treat that checkout
    // transformation as equivalent; every JSON token, field order, value, and
    // XDR byte remains subject to an exact comparison.
    let existing_canonical = canonical_fixture_text(&existing);

    if existing_canonical != rendered {
        panic!(
            "fixture drift detected in {}\n\
             The contract's observable behaviour no longer matches the committed fixture.\n\
             If the change is intentional, regenerate with:\n\
             \n    UPDATE_FIXTURES=1 cargo test --test sdk_fixtures\n\
             \nand review the diff before committing.\n\
             \n--- committed ---\n{}\n--- generated ---\n{}",
            path.display(),
            truncate(&existing_canonical),
            truncate(&rendered)
        );
    }
}

fn canonical_fixture_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn truncate(s: &str) -> String {
    const MAX: usize = 4000;
    if s.len() <= MAX {
        s.to_string()
    } else {
        format!("{}\n… ({} more bytes)", &s[..MAX], s.len() - MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::canonical_fixture_text;

    #[test]
    fn fixture_comparison_normalizes_only_windows_line_endings() {
        assert_eq!(
            canonical_fixture_text("{\r\n  \"ok\": true\r\n}\r\n"),
            "{\n  \"ok\": true\n}\n"
        );
        assert_ne!(
            canonical_fixture_text("{\r\n  \"ok\": false\r\n}\r\n"),
            "{\n  \"ok\": true\n}\n",
            "normalization must not hide fixture content drift"
        );
    }
}

/// Standard envelope wrapped around every fixture file.
pub fn envelope(name: &str, purpose: &str, scenarios: Vec<Json>) -> Json {
    let mut obj = JsonObj::new();
    obj.push("$schema_version", Json::Num(1));
    obj.push("fixture", Json::str(name));
    obj.push("purpose", Json::str(purpose));
    obj.push("contract", Json::str(CONTRACT_ADDRESS));
    obj.push(
        "generator",
        Json::str("tests/sdk_fixtures.rs (cargo test --test sdk_fixtures)"),
    );
    obj.push(
        "notes",
        Json::Arr(vec![
            Json::str(
                "Generated from live contract invocations in the Soroban test host; \
                 values are rendered from wire-level ScVal/XDR, never hand-written.",
            ),
            Json::str(
                "All addresses are synthetic, derived from SHA-256(\"aegis-fixture/<label>\"). \
                 They have no private keys and contain no real user data.",
            ),
            Json::str(
                "128-bit and 64-bit integers are encoded as decimal strings to stay \
                 lossless in JSON consumers that use IEEE-754 numbers.",
            ),
        ]),
    );
    obj.push("scenarios", Json::Arr(scenarios));
    obj.build()
}

/// Builds one scenario entry: a stable `id`, a human `description`, and a
/// payload map of named steps/values.
pub struct Scenario {
    id: String,
    description: String,
    fields: Vec<(String, Json)>,
}

impl Scenario {
    pub fn new(id: &str, description: &str) -> Scenario {
        Scenario {
            id: id.to_string(),
            description: description.to_string(),
            fields: Vec::new(),
        }
    }

    pub fn set(mut self, key: &str, value: Json) -> Scenario {
        self.fields.push((key.to_string(), value));
        self
    }

    pub fn build(self) -> Json {
        let mut obj = JsonObj::new();
        obj.push("id", Json::Str(self.id));
        obj.push("description", Json::Str(self.description));
        for (k, v) in self.fields {
            obj.push_owned(k, v);
        }
        obj.build()
    }
}

/// Asserts that scenario ids within a fixture file are unique, so downstream
/// SDKs can safely address a scenario by id.
pub fn assert_unique_ids(scenarios: &[Json]) {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for s in scenarios {
        if let Json::Obj(entries) = s {
            if let Some((_, Json::Str(id))) = entries.iter().find(|(k, _)| k == "id") {
                *seen.entry(id.clone()).or_insert(0) += 1;
            }
        }
    }
    let dupes: Vec<_> = seen.iter().filter(|(_, n)| **n > 1).collect();
    assert!(dupes.is_empty(), "duplicate scenario ids: {dupes:?}");
}
