# Local Deployment Guide

This guide takes you from a fresh clone to a deployed, initialized Aegis
contract running against a local Stellar network. It documents every Makefile
target, every environment variable, the Soroban/Stellar CLI commands used, and
the errors contributors hit most often.

**Audience:** contributors who want to build, test, deploy, and interact with
the Aegis RWA contracts locally.

---

## Table of contents

1. [Deployment assumptions](#1-deployment-assumptions)
2. [Prerequisites](#2-prerequisites)
3. [Quick start](#3-quick-start)
4. [Environment variables](#4-environment-variables)
5. [Makefile command reference](#5-makefile-command-reference)
6. [Helper scripts](#6-helper-scripts)
7. [Soroban / Stellar CLI usage](#7-soroban--stellar-cli-usage)
8. [Post-deployment: initializing the protocol](#8-post-deployment-initializing-the-protocol)
9. [Common errors and fixes](#9-common-errors-and-fixes)
10. [Deploying to Testnet](#10-deploying-to-testnet)

---

## 1. Deployment assumptions

Read this section first — most local deployment failures trace back to one of
these assumptions being violated.

| # | Assumption | Why it matters |
|---|---|---|
| 1 | **The WASM target is `wasm32v1-none`, not `wasm32-unknown-unknown`.** | `soroban-sdk` 26.x refuses to build for `wasm32-unknown-unknown` on Rust 1.82+, because that target enables the `reference-types` and `multi-value` WASM features the Soroban environment does not support. The build aborts from `soroban-sdk`'s `build.rs`. `wasm32v1-none` requires Rust 1.84+. |
| 2 | **This repo builds a single contract crate.** | `Cargo.toml` declares one package, `aegis-contracts`, with `crate-type = ["cdylib", "rlib"]`. The compiled artifact is `target/wasm32v1-none/release/aegis_contracts.wasm` (dashes in the package name become underscores in the artifact). |
| 3 | **Tests do not require a network, a wasm target, or Docker.** | The suite in `src/test.rs` uses `soroban_sdk::testutils` with an in-memory `Env`. `make test` compiles for your **native** target. You can contribute logic and tests without ever installing Docker or the CLI. |
| 4 | **`initialize` must be called exactly once, right after deploy.** | Until `initialize(admin)` runs there is no admin in storage, and every privileged call fails with `NotInitialized` (2000). A second call fails with `AlreadyInitialized` (1000). |
| 5 | **The deployer account must exist and be funded on the target network.** | Deployment is a transaction; it needs a funded source account. Local quickstart and Testnet both expose a friendbot, reachable via `stellar keys fund`. |
| 6 | **The network passphrase must match the running network exactly.** | A mismatched passphrase produces signatures the network rejects. Local quickstart uses `Standalone Network ; February 2017`; Testnet uses `Test SDF Network ; September 2015`. |
| 7 | **Local network state is ephemeral.** | The quickstart container runs with `--rm`. Stopping it discards all ledger state, so the contract id in `.aegis-deploy` becomes invalid and you must redeploy. |
| 8 | **The admin bypasses all role checks.** | `require_role` returns early when the caller is the supreme admin, so a freshly initialized contract needs no extra role grants for you to mint, whitelist, and pause while testing. |

---

## 2. Prerequisites

| Tool | Version | Needed for | Install |
|---|---|---|---|
| Rust | **1.84+** | building and testing | <https://rustup.rs/> |
| `wasm32v1-none` target | — | building the contract WASM | `rustup target add wasm32v1-none` |
| Stellar CLI | 21.0.0+ (`stellar`) | deploy / invoke / local network | `cargo install --locked stellar-cli` |
| Docker | any recent | running the local network | <https://docs.docker.com/get-docker/> |
| GNU Make | any | the `make` shortcuts (optional) | preinstalled on macOS/Linux |

> **Note on the CLI name.** The `soroban` binary was renamed to `stellar` in
> CLI v21. This repo defaults to `stellar`. If you are still on the legacy
> binary, set `STELLAR=soroban` in your `.env` and every command below keeps
> working.

> **Windows contributors.** GNU Make is not installed by default. Either use
> WSL2 (recommended — the shell scripts assume a POSIX shell) or run the raw
> `cargo` / `stellar` commands shown in each section.

Verify your setup:

```bash
rustc --version                        # must be >= 1.84
rustup target list --installed | grep wasm32v1-none
stellar --version
docker info                            # daemon must be reachable
```

---

## 3. Quick start

The fastest path from clone to a live, initialized contract:

```bash
git clone https://github.com/felladaniel36-hash/aegis-contracts.git
cd aegis-contracts

cp .env.example .env          # 1. create local configuration
rustup target add wasm32v1-none

make test                     # 2. verify the toolchain (no network needed)

./scripts/setup_local.sh      # 3. start local network + fund a deployer key
./scripts/deploy_local.sh     # 4. build, deploy, and initialize
```

`deploy_local.sh` prints the contract id and writes it to `.aegis-deploy`.
Paste it into `CONTRACT_ID` in your `.env` so later `make` targets pick it up:

```bash
make invoke-status CONTRACT_ID=$(cat .aegis-deploy)
```

Prefer doing it step by step? See
[Soroban / Stellar CLI usage](#7-soroban--stellar-cli-usage) for the raw
commands each script runs.

---

## 4. Environment variables

Copy `.env.example` to `.env` and edit as needed. The Makefile auto-loads
`.env` when it exists, and the shell scripts read it too. **Values already set
in your shell environment take precedence over the file**, so one-off
overrides work as expected:

```bash
NETWORK=testnet ./scripts/deploy_local.sh
make deploy SOURCE_ACCOUNT=my-other-key
```

`.env` is git-ignored. `.env.example` is explicitly un-ignored so the template
stays in version control. **Never commit real secret keys.**

### Build configuration

| Variable | Default | Description |
|---|---|---|
| `WASM_TARGET` | `wasm32v1-none` | Rust compilation target for the contract. Do not change unless you are pinned to Rust ≤ 1.81, in which case use `wasm32-unknown-unknown`. |
| `CONTRACT_NAME` | `aegis_contracts` | Artifact name used to locate the built WASM. Derived from the `name` in `Cargo.toml` with `-` replaced by `_`. Change only if the package is renamed. |

### CLI configuration

| Variable | Default | Description |
|---|---|---|
| `STELLAR` | `stellar` | Name of the CLI binary. Set to `soroban` for CLI versions older than 21.0.0. |

### Network configuration

| Variable | Default | Description |
|---|---|---|
| `NETWORK` | `local` | Network alias as registered in the CLI config (`stellar network ls`). Typical values: `local`, `testnet`, `futurenet`. |
| `RPC_URL` | `http://localhost:8000/rpc` | JSON-RPC endpoint. Testnet: `https://soroban-testnet.stellar.org`. |
| `NETWORK_PASSPHRASE` | `Standalone Network ; February 2017` | Must match the network exactly or all transactions fail signature verification. Testnet: `Test SDF Network ; September 2015`. |

### Identity

| Variable | Default | Description |
|---|---|---|
| `SOURCE_ACCOUNT` | `aegis-admin` | CLI identity alias that signs deploy and invoke transactions. Created and funded by `make fund` / `setup_local.sh`. Prefer an alias over a raw secret key so no secret is written to `.env`. |

### Deployment state

| Variable | Default | Description |
|---|---|---|
| `CONTRACT_ID` | *(empty)* | Contract id (`C...`) produced by a deploy. Populated in `.aegis-deploy`; copy it into `.env` to make `make initialize` / `make invoke-status` work without arguments. |
| `ADMIN_ADDRESS` | *(empty)* | Address (`G...`) that becomes the supreme admin via `initialize`. Defaults to the `SOURCE_ACCOUNT` address in `deploy_local.sh`. Get it with `stellar keys address aegis-admin`. |
| `AUTO_INITIALIZE` | `1` | When `1`, `deploy_local.sh` calls `initialize` immediately after deploying. Set to `0` to deploy without initializing. |

---

## 5. Makefile command reference

Run `make help` at any time for this list plus your currently resolved
configuration values.

### Build

| Target | What it does |
|---|---|
| `make build` | Verifies the wasm target is installed, then compiles the release WASM to `target/$(WASM_TARGET)/release/aegis_contracts.wasm`. |
| `make build-legacy` | Builds for `wasm32-unknown-unknown`. **Only works on Rust ≤ 1.81**; prints a warning first. Provided for contributors pinned to an old toolchain. |
| `make optimize` | Runs `make build`, then shrinks the WASM with `stellar contract optimize`. Produces `aegis_contracts.optimized.wasm`. |
| `make check-target` | Preflight: fails with a `rustup target add` hint if the wasm target is missing. |

### Test and quality

| Target | What it does |
|---|---|
| `make test` | Runs the full unit suite (`cargo test`) on the native target. No network, Docker, or wasm target required. |
| `make test-verbose` | Same, with `--nocapture` so `println!` output is shown. |
| `make fmt` | Formats all sources in place (`cargo fmt --all`). |
| `make fmt-check` | Fails if any file is unformatted. Use before opening a PR. |
| `make clippy` | Lints with warnings escalated to errors (`-D warnings`). |
| `make ci` | Runs the full PR gate: `fmt-check` + `clippy` + `test` + `build`. |
| `make all` | `fmt` + `test` + `build`. |
| `make clean` | `cargo clean` plus removal of `.aegis-deploy`. |

### Local network

| Target | What it does |
|---|---|
| `make network-up` | Starts the Stellar quickstart container (`stellar container start local`). |
| `make network-down` | Stops it (`stellar container stop local`). All ledger state is lost. |
| `make network-add` | Registers the `$(NETWORK)` alias in the CLI config with `$(RPC_URL)` and `$(NETWORK_PASSPHRASE)`. |
| `make fund` | Creates `$(SOURCE_ACCOUNT)` if it does not exist, then funds it via friendbot. |
| `make check-cli` | Preflight: fails with an install hint if the Stellar CLI is missing. |

### Deploy and interact

| Target | What it does |
|---|---|
| `make deploy` | Builds, deploys to `$(NETWORK)`, and writes the contract id to `.aegis-deploy`. |
| `make initialize` | Calls `initialize(admin)`. Requires `CONTRACT_ID` and `ADMIN_ADDRESS` (from `.env` or the command line). |
| `make invoke-status` | Reads back `get_total_supply`, `is_paused`, and `get_asset_status`. Handy smoke test after deploying. |
| `make verify` | Prints the deployed contract's interface as the network sees it (`stellar contract info interface`). |

Every target accepts inline overrides:

```bash
make build WASM_TARGET=wasm32-unknown-unknown
make deploy NETWORK=testnet SOURCE_ACCOUNT=my-testnet-key
```

---

## 6. Helper scripts

Both scripts live in `scripts/`, share `scripts/lib/common.sh`, are idempotent,
and support `--help`. They fail fast with an actionable `fix:` hint rather than
a raw stack trace.

### `scripts/setup_local.sh`

Prepares the environment. Steps:

1. Checks Rust, the wasm target, and the Stellar CLI are present.
2. Starts the quickstart container and polls the RPC `getHealth` endpoint
   until it reports `healthy` (this can take a few minutes on first run).
3. Registers the network alias in the CLI config.
4. Creates the deployer identity if missing and funds it.

```bash
./scripts/setup_local.sh                 # full setup
./scripts/setup_local.sh --no-network    # skip Docker; configure CLI + key only
```

Use `--no-network` when targeting Testnet or when the container is already up.

### `scripts/deploy_local.sh`

Builds, deploys, and initializes. Steps:

1. Checks the CLI, wasm target, deployer identity, and RPC health.
2. Builds the release WASM.
3. Deploys it and writes the contract id to `.aegis-deploy`.
4. Calls `initialize(admin)` unless disabled.

```bash
./scripts/deploy_local.sh                # build + deploy + initialize
./scripts/deploy_local.sh --skip-build   # reuse the existing WASM
./scripts/deploy_local.sh --no-init      # deploy only
```

If `initialize` reports `Error(Contract, #1000)` the script warns rather than
failing — that code means the contract was already initialized, which is
expected when re-running against an existing deployment.

---

## 7. Soroban / Stellar CLI usage

What the scripts do, as raw commands you can run yourself.

### Start a local network

```bash
stellar container start local
```

Equivalent to running the quickstart image directly:

```bash
docker run --rm -it -p 8000:8000 --name stellar \
  stellar/quickstart:testing --local --enable rpc,horizon,core
```

Check health (the container is not usable until this returns `healthy`):

```bash
curl -s -X POST http://localhost:8000/rpc \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'
```

### Configure the network and identity

```bash
stellar network add local \
  --rpc-url "http://localhost:8000/rpc" \
  --network-passphrase "Standalone Network ; February 2017"

stellar keys generate --global aegis-admin --network local
stellar keys fund aegis-admin --network local
stellar keys address aegis-admin        # -> G...
```

### Build, deploy, initialize

```bash
cargo build --target wasm32v1-none --release

stellar contract deploy \
  --wasm target/wasm32v1-none/release/aegis_contracts.wasm \
  --source-account aegis-admin \
  --network local
# -> C... (the contract id)

stellar contract invoke \
  --id <CONTRACT_ID> --source-account aegis-admin --network local \
  -- initialize --admin $(stellar keys address aegis-admin)
```

### Discover the interface

Anything after `--` is parsed against the contract's own schema, so the CLI can
list functions and their arguments for you:

```bash
stellar contract invoke --id <CONTRACT_ID> --source-account aegis-admin \
  --network local -- --help

stellar contract info interface --id <CONTRACT_ID> --network local
```

### Exercising the protocol

A representative end-to-end flow. `ADMIN` below is the admin `G...` address;
the admin bypasses role checks, so no role grant is needed first.

```bash
ADMIN=$(stellar keys address aegis-admin)
CID=$(cat .aegis-deploy)
INV=$(stellar keys address investor-1)

# Compliance: whitelist an investor
stellar contract invoke --id $CID --source-account aegis-admin --network local \
  -- whitelist_user --admin $ADMIN --user $INV

# Issuance: mint to the whitelisted investor
stellar contract invoke --id $CID --source-account aegis-admin --network local \
  -- mint_asset --admin $ADMIN --to $INV --amount 1000

# Reads
stellar contract invoke --id $CID --source-account aegis-admin --network local \
  -- get_balance_of --address $INV
stellar contract invoke --id $CID --source-account aegis-admin --network local \
  -- get_total_supply
stellar contract invoke --id $CID --source-account aegis-admin --network local \
  -- get_investor_eligibility --investor $INV

# Roles (Admin cannot be assigned here — use transfer_admin/accept_admin)
stellar contract invoke --id $CID --source-account aegis-admin --network local \
  -- set_role --admin $ADMIN --target $INV --role ComplianceOfficer

# Governance: 2-step supply cap amendment
stellar contract invoke --id $CID --source-account aegis-admin --network local \
  -- propose_supply_cap --admin $ADMIN --proposed_cap 1000000
stellar contract invoke --id $CID --source-account aegis-admin --network local \
  -- accept_supply_cap --admin $ADMIN

# Emergency pause / unpause
stellar contract invoke --id $CID --source-account aegis-admin --network local \
  -- pause --caller $ADMIN
stellar contract invoke --id $CID --source-account aegis-admin --network local \
  -- unpause --caller $ADMIN
```

Note the argument names are the Rust parameter names (`--admin`, `--to`,
`--amount`, `--proposed_cap`), and enum variants are passed by name
(`ComplianceOfficer`, `AssetManager`, `EmergencyOfficer`).

### Watch events

```bash
stellar events --start-ledger 1 --id <CONTRACT_ID> --network local
```

Event payload shapes are documented in [events.md](events.md).

---

## 8. Post-deployment: initializing the protocol

A freshly deployed contract is not usable until it is configured. Recommended
order:

1. **`initialize(admin)`** — sets the supreme admin. Required; everything else
   fails until this runs.
2. **`set_role(admin, target, role)`** — delegate `ComplianceOfficer`,
   `AssetManager`, or `EmergencyOfficer` as needed. Optional for solo local
   testing, since the admin bypasses role checks. `Role::Admin` is rejected
   here by design — use `transfer_admin` / `accept_admin`.
3. **`update_asset_metadata(caller, name, symbol, uri)`** — set the asset's
   display metadata. Requires `AssetManager` or admin.
4. **`propose_supply_cap` + `accept_supply_cap`** — optional. A cap of `0`
   (the default) means unbounded minting.
5. **`propose_holding_cap` + `accept_holding_cap`** — optional per-investor
   cap. `0` means unrestricted.
6. **`whitelist_user(admin, user)`** — required before any address can receive
   a mint or transfer.

Background reading: [admin-roles.md](admin-roles.md),
[supply-cap-governance.md](supply-cap-governance.md),
[investor-holding-restrictions.md](investor-holding-restrictions.md),
[emergency-pause.md](emergency-pause.md).

---

## 9. Common errors and fixes

### 9.1 Build and toolchain errors

**`Rust compiler 1.82+ with target 'wasm32-unknown-unknown' is unsupported by the Soroban Environment, use 'wasm32v1-none' available with Rust 1.84+`**

The single most common failure. Modern Rust enables `reference-types` and
`multi-value` for `wasm32-unknown-unknown`, which Soroban rejects.

*Fix:* build for the supported target.
```bash
rustup target add wasm32v1-none
make build                      # already defaults to wasm32v1-none
```
If you must stay on `wasm32-unknown-unknown`, pin to Rust 1.81 or earlier and
use `make build-legacy`.

---

**`error[E0463]: can't find crate for 'core'` / `the wasm32v1-none target may not be installed`**

*Fix:*
```bash
rustup target add wasm32v1-none
```
`make build` runs this check first and tells you the exact command.

---

**`ERROR: Rust target 'wasm32v1-none' is not installed.`**

Emitted by `make check-target`. Same fix as above.

---

**`error: linking with 'cc' failed` when running `make test`**

Tests build for your native target and need a C linker.

*Fix:* install build essentials — `sudo apt install build-essential` (Debian/
Ubuntu) or `xcode-select --install` (macOS).

---

**The build is killed with `signal: 9, SIGKILL`**

The compiler ran out of memory — common in small containers and CI runners.

*Fix:* reduce parallelism and debug info:
```bash
CARGO_PROFILE_DEV_DEBUG=0 cargo test -j1
```

---

**`warning: use of deprecated method 'soroban_sdk::events::Events::publish'`**

Known, tracked, and harmless. The contract still builds and all tests pass.
Migration to the `#[contractevent]` macro is tracked in
[contributor-experience-review.md](contributor-experience-review.md).

---

### 9.2 CLI and network errors

**`stellar: command not found`** (or `soroban: command not found`)

*Fix:*
```bash
cargo install --locked stellar-cli
# legacy binary instead? then in .env:
STELLAR=soroban
```

---

**`error sending request for url (http://localhost:8000/rpc)` / connection refused**

The local network is not running, still starting, or on a different port.

*Fix:*
```bash
make network-up
docker ps                      # confirm the 'stellar' container is up
curl -s -X POST http://localhost:8000/rpc -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'
```
First start can take several minutes to sync. `setup_local.sh` polls for you.

---

**`ERR No response from RPC at ...` from a script**

Same root cause; the script's preflight caught it before wasting a build.
Start the network, wait for `healthy`, retry.

---

**Port 8000 already allocated**

*Fix:* stop the conflicting container, or remap:
```bash
make network-down
docker rm -f stellar
```

---

**Transactions rejected with a signature/auth error, or `txBadAuth`**

`NETWORK_PASSPHRASE` does not match the running network.

*Fix:* confirm the alias is registered with the right passphrase.
```bash
stellar network ls --long
# local   -> Standalone Network ; February 2017
# testnet -> Test SDF Network ; September 2015
make network-add            # re-register with the values from .env
```

---

**`account not found` / `Account not found: G...` during deploy**

The deployer account does not exist on-chain yet.

*Fix:*
```bash
make fund
# or: stellar keys fund aegis-admin --network local
```
Note that funding is per-network: an account funded on Testnet does not exist
on your local network, and local state is wiped whenever the container stops.

---

**`Identity 'aegis-admin' does not exist`**

*Fix:*
```bash
./scripts/setup_local.sh --no-network
# or: stellar keys generate --global aegis-admin --network local
```

---

**Contract id suddenly invalid / `Contract not found` after a restart**

Local ledger state is ephemeral (assumption 7). The container was restarted.

*Fix:* redeploy and update `CONTRACT_ID`.
```bash
./scripts/deploy_local.sh
```

---

**`No WASM at target/.../aegis_contracts.wasm`**

Either the build has not run, or `CONTRACT_NAME` / `WASM_TARGET` do not match
what was produced.

*Fix:* run `make build`, then check the values in `.env`. The artifact name is
the Cargo package name with dashes converted to underscores.

---

### 9.3 Contract runtime errors

Contract errors surface as `Error(Contract, #<code>)`. Codes are grouped by
category; the full standard is in [error-codes.md](error-codes.md).

| Code | Name | Meaning and fix |
|---|---|---|
| 1000 | `AlreadyInitialized` | `initialize` was already called. Not a problem when re-running a deploy script against an existing contract; deploy fresh if you need a clean instance. |
| 2000 | `NotInitialized` | No admin in storage. Run `initialize` first. |
| 2001 | `NoPendingAdminTransfer` | `accept_admin` called with no outstanding `transfer_admin`. |
| 3000 | `Unauthorized` | The caller lacks the required role. Check `get_role_of`, and confirm `--source-account` signs for the address passed as the `admin`/`caller` argument. |
| 3001 | `CannotAssignAdminRole` | `set_role` cannot grant `Admin`. Use `transfer_admin` then `accept_admin`. |
| 3002 | `NoRoleToRevoke` | Target has no role assigned. |
| 3003 | `NotPendingCandidate` | Caller is not the recorded admin candidate. |
| 3004 | `ContractPaused` | The contract is paused. Run `unpause --caller <admin>` (admin only). |
| 3005 | `AlreadyPaused` | Pause called while already paused. |
| 3006 | `NotPaused` | Unpause called while not paused. |
| 4000 | `SenderNotWhitelisted` | Whitelist the sender: `whitelist_user --admin <A> --user <sender>`. |
| 4001 | `ReceiverNotWhitelisted` | Whitelist the receiver. The most common mint failure — recipients must be whitelisted **before** minting. |
| 5000 | `InvalidAmount` | Amount must be strictly greater than zero. |
| 5001 | `InsufficientBalance` | Sender balance is too low. |
| 6000 | `AssetNotActive` | Asset status is not `Active`. Restore with `set_asset_status --caller <A> --new_status Active`. |
| 6001 | `InvalidAssetStatusTransition` | Lifecycle rules reject the transition. `Retired` is terminal; a status cannot transition to itself. |
| 6002 | `AssetMetadataUpdateBlocked` | Metadata is frozen in `Retired`/`Blocked` status. |

Some governance paths use `panic!`/`assert!` instead of typed errors and
surface as host panics with a readable message, for example
`Proposed cap equals the active cap — no change requested` or
`No pending supply cap proposal to accept`. These come from the supply-cap and
holding-cap 2-step flows.

**Supply/holding cap panics on mint.** If minting fails with a cap message,
inspect the active limits — a cap of `0` means unlimited:

```bash
stellar contract invoke --id $CID --source-account aegis-admin --network local \
  -- get_supply_cap
stellar contract invoke --id $CID --source-account aegis-admin --network local \
  -- get_holding_cap
```

---

## 10. Deploying to Testnet

The same tooling targets Testnet — only the environment changes.

```bash
stellar network add testnet \
  --rpc-url "https://soroban-testnet.stellar.org" \
  --network-passphrase "Test SDF Network ; September 2015"

stellar keys generate --global aegis-testnet --network testnet
stellar keys fund aegis-testnet --network testnet
```

Then either set the values in `.env`:

```dotenv
NETWORK=testnet
RPC_URL=https://soroban-testnet.stellar.org
NETWORK_PASSPHRASE=Test SDF Network ; September 2015
SOURCE_ACCOUNT=aegis-testnet
```

or override per command:

```bash
make deploy NETWORK=testnet SOURCE_ACCOUNT=aegis-testnet
./scripts/deploy_local.sh --no-init    # after `setup_local.sh --no-network`
```

Use `make optimize` before any non-local deployment to reduce the WASM size and
therefore the deployment fee.

> Testnet is reset periodically by SDF. Treat deployed contract ids as
> temporary and never use Testnet keys on Mainnet.

---

## Related documentation

- [Contract API specification](contract-spec.md)
- [Protocol architecture](architecture.md)
- [Contract error codes](error-codes.md)
- [Events reference](events.md)
- [Admin roles and permissions](admin-roles.md)
- [Dashboard local troubleshooting](dashboard-troubleshooting.md) — front-end counterpart to this guide
- [Contributing guidelines](../CONTRIBUTING.md)
