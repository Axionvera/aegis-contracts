# Failing CI Response Guide (Aegis Contracts)

How to reproduce and fix failing checks on Aegis contract pull requests.

> **Why this matters:** a PR with failing checks may not be approved or merged —
> the repository's automation treats red checks as a blocker. This guide helps
> you get from a red ✗ to a green ✓ before asking a maintainer.

## How checks run here

This repo does **not** run Rust/Soroban builds directly in a `.github/workflows`
build job. Instead, the workflows (`auto-trigger.yml`, `trigger-auto-assign.yml`)
dispatch events to the central automation repo
(`Axionvera/pocketpay-issue-automation`), which performs the actual validation
(build, test, lint). A red check therefore means the automation reported a
failure — you still reproduce and fix it **locally** with the same toolchain.

## Local commands (run these first)

```bash
# 1. Add the WASM target (one-time)
rustup target add wasm32-unknown-unknown

# 2. Build the contract (matches `make build`)
make build
#    equivalent: cargo build --target wasm32-unknown-unknown --release

# 3. Run the test suite (matches `make test`)
make test
#    equivalent: cargo test

# 4. Format & lint
make fmt            # cargo fmt --all
cargo clippy --all-targets --all-features   # if the project uses clippy

# 5. Optimize the WASM (optional, larger artifact)
make optimize
```

If any of these fail locally, they will also fail in automation.

---

## Failure categories

### 1. Rust compilation failures (`cargo build` / `cargo test` errors)
**Symptoms:** `error[E####]`, `cannot find`, `mismatched types`,
`unused variable` (with `#![deny(warnings)]`).

**Common causes & fixes:**
- **Missing `wasm32-unknown-unknown` target** → `rustup target add wasm32-unknown-unknown`.
- **SDK type/API drift** (e.g. `soroban-sdk` method renames, `Events::all()`
  tuple shape, `symbol_short!` 9-char limit) → align your code with the pinned
  `soroban-sdk` version in `Cargo.toml`.
- **`#[contract]` / `#[contractimpl]` client generation** → ensure the generated
  `XxxClient` is imported where used (`use crate::{Contract, ContractClient};`).
- **Warnings treated as errors** → fix the warning (don't `#[allow]` to hide it
  unless justified).

### 2. Soroban failures
**Symptoms:** WASM build succeeds but contract fails to invoke, or
`soroban contract optimize` errors on the artifact.

**Common causes & fixes:**
- **Wrong wasm binary name/path** → `Makefile` expects
  `target/wasm32-unknown-unknown/release/aegis_contracts.wasm`. If your
  `Cargo.toml` `package.name` differs, fix `Makefile` *and* the path.
- **`optimize` step fails** → ensure `soroban` CLI is installed and the source
  WASM exists (`make build` first).
- **Contract logic panics in tests** → read the test panic message; it usually
  names the offending function/line.

### 3. Makefile issues
**Symptoms:** `make: *** No rule to make target` or `recipe for target failed`.

**Common causes & fixes:**
- **Tab vs spaces** → Makefiles require **tabs** for recipe lines. If you edited
  the Makefile, ensure indentation is a literal tab, not spaces.
- **Missing binary/path mismatch** → verify the artifact name matches
  `Cargo.toml`'s `package.name` (the Makefile hard-codes
  `aegis_contracts.wasm`).
- **Tool not on PATH** → `cargo`, `rustup`, `soroban` must be available in the
  shell running `make`.

### 4. Dependency failures
**Symptoms:** `failed to load source`, `version requirement ... not found`,
network/registry errors during `cargo build`.

**Common causes & fixes:**
- **Pinned SDK missing from registry/cache** → run `cargo update -p soroban-sdk`
  only if the lockfile allows; otherwise match the `Cargo.toml` pin exactly.
- **`Cargo.lock` drift** → commit `Cargo.lock` for reproducible builds; if CI
  regenerates it, ensure your local lock matches.
- **Offline/registry hiccup** → retry; if persistent, check
  `CARGO_NET_*` / network access.

### 5. Workflow dispatch failures
**Symptoms:** the `Trigger Auto Merge Automation` / `Trigger Auto Assign` jobs
fail, or no checks appear at all.

**Common causes & fixes:**
- These jobs only **dispatch** to the external automation repo; a failure here is
  usually a token/secret (`AXIONVERA_AUTOMATION_TOKEN`) or dispatch permission
  issue — **not something you fix in code**.
- If checks never appear, confirm the PR is opened from a **branch** (not a
  draft stuck in `ready_for_review`), and that the head repo is correct.
- You cannot resolve token/permission failures yourself — flag it to a
  maintainer, but still ensure your code builds/tests locally.

---

## Reproduce → Fix → Verify loop

1. Run `make build` and `make test` locally. Capture the exact error.
2. Fix the root cause (don't mask warnings/errors to force green).
3. Re-run both commands until fully green locally.
4. `git add` the fix, commit, push. The automation re-runs on
   `synchronize` (new push) and updates the check status.
5. Confirm the checks flip to green on the PR before requesting review.

## When to ask a maintainer
- The failure is in the **external automation** (dispatch/token), not your code.
- A check is red but `make build` + `make test` are green locally (possible
  toolchain/version mismatch in automation).
- You believe the pinned `soroban-sdk` version itself is broken.

In all cases, attach your local build/test output — it speeds up the fix.
