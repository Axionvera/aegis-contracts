#!/usr/bin/env bash
#
# deploy_local.sh — build, deploy, and initialize the Aegis contract.
#
# Performs, in order:
#   1. Preflight checks (Stellar CLI, wasm target, RPC health).
#   2. Builds the contract WASM (skippable with --skip-build).
#   3. Deploys it and records the contract id in .aegis-deploy.
#   4. Calls initialize(admin) unless AUTO_INITIALIZE=0 or --no-init.
#
# Usage:
#   ./scripts/deploy_local.sh [--skip-build] [--no-init] [--help]
#
# Configuration comes from .env (see .env.example). Override per-invocation:
#   NETWORK=testnet SOURCE_ACCOUNT=my-key ./scripts/deploy_local.sh
#
# Documented in docs/local-deployment.md
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$SCRIPT_DIR/lib/common.sh"

SKIP_BUILD=0
NO_INIT=0
for arg in "$@"; do
  case "$arg" in
    --skip-build) SKIP_BUILD=1 ;;
    --no-init) NO_INIT=1 ;;
    -h|--help) sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "Unknown argument: $arg" "Run '$0 --help' for usage." ;;
  esac
done

load_env
apply_defaults

# ─── 1. Preflight ─────────────────────────────────────────────────────────────

info "Preflight checks"
require_stellar_cli
require_wasm_target
"$STELLAR" keys address "$SOURCE_ACCOUNT" >/dev/null 2>&1 || die \
  "Identity '$SOURCE_ACCOUNT' does not exist in the Stellar CLI config." \
  "./scripts/setup_local.sh   (or: $STELLAR keys generate --global $SOURCE_ACCOUNT --network $NETWORK)"
require_rpc_healthy

# ─── 2. Build ─────────────────────────────────────────────────────────────────

if [ "$SKIP_BUILD" -eq 1 ]; then
  info "Skipping build (--skip-build)"
  [ -f "$WASM_PATH" ] || die "No WASM at $WASM_PATH" "Re-run without --skip-build."
else
  info "Building contract (target: $WASM_TARGET)"
  ( cd "$REPO_ROOT" && cargo build --target "$WASM_TARGET" --release ) || die \
    "Contract build failed." \
    "If the error mentions reference-types/multi-value, set WASM_TARGET=wasm32v1-none in .env"
fi

[ -f "$WASM_PATH" ] || die \
  "Expected WASM not found at $WASM_PATH" \
  "Check CONTRACT_NAME in .env matches the package name in Cargo.toml (dashes -> underscores)."
ok "WASM ready: $WASM_PATH ($(wc -c < "$WASM_PATH") bytes)"

# ─── 3. Deploy ────────────────────────────────────────────────────────────────

info "Deploying to network '$NETWORK'"
CONTRACT_ID_OUT="$("$STELLAR" contract deploy \
  --wasm "$WASM_PATH" \
  --source-account "$SOURCE_ACCOUNT" \
  --network "$NETWORK" 2>/dev/null | tail -n1 | tr -d '[:space:]')" || die \
  "Deployment failed." \
  "Verify the account is funded ('stellar keys fund $SOURCE_ACCOUNT --network $NETWORK') and the network passphrase matches."

case "$CONTRACT_ID_OUT" in
  C*) ;;
  *) die "Deploy did not return a contract id (got: '${CONTRACT_ID_OUT:-<empty>}')." \
         "Re-run with the CLI's verbose flag: $STELLAR -v contract deploy ..." ;;
esac

printf '%s' "$CONTRACT_ID_OUT" > "$DEPLOY_OUT"
ok "Deployed: $CONTRACT_ID_OUT"
ok "Contract id written to $DEPLOY_OUT"

# ─── 4. Initialize ────────────────────────────────────────────────────────────

if [ "$NO_INIT" -eq 1 ] || [ "$AUTO_INITIALIZE" != "1" ]; then
  info "Skipping initialize"
  printf '     Run later: make initialize CONTRACT_ID=%s ADMIN_ADDRESS=<G...>\n' "$CONTRACT_ID_OUT"
  exit 0
fi

ADMIN="${ADMIN_ADDRESS:-$($STELLAR keys address "$SOURCE_ACCOUNT")}"
info "Initializing with admin $ADMIN"

if "$STELLAR" contract invoke \
    --id "$CONTRACT_ID_OUT" \
    --source-account "$SOURCE_ACCOUNT" \
    --network "$NETWORK" \
    -- initialize --admin "$ADMIN"; then
  ok "Contract initialized. Admin: $ADMIN"
else
  warn "initialize failed."
  warn "If the error is 'Error(Contract, #1000)' the contract is already initialized —"
  warn "that is expected when re-running against an existing deployment."
fi

echo
ok "Done."
printf '     CONTRACT_ID=%s\n' "$CONTRACT_ID_OUT"
printf '     Add that value to .env so make targets pick it up automatically.\n'
printf '     Inspect state: make invoke-status CONTRACT_ID=%s\n' "$CONTRACT_ID_OUT"
