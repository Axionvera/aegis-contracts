#!/usr/bin/env bash
#
# setup_local.sh — prepare a local Soroban development environment.
#
# Idempotent: safe to re-run. Performs, in order:
#   1. Preflight checks (Rust toolchain, wasm target, Stellar CLI, Docker).
#   2. Starts the local Stellar quickstart network (unless --no-network).
#   3. Registers the network alias in the Stellar CLI config.
#   4. Creates and funds the deployer identity.
#
# Usage:
#   ./scripts/setup_local.sh [--no-network] [--help]
#
# Configuration is read from .env (see .env.example) and can be overridden
# per-invocation, e.g. NETWORK=testnet ./scripts/setup_local.sh --no-network
#
# Documented in docs/local-deployment.md
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$SCRIPT_DIR/lib/common.sh"

START_NETWORK=1
for arg in "$@"; do
  case "$arg" in
    --no-network) START_NETWORK=0 ;;
    -h|--help) sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "Unknown argument: $arg" "Run '$0 --help' for usage." ;;
  esac
done

load_env
apply_defaults

info "Configuration"
printf '     NETWORK        = %s\n' "$NETWORK"
printf '     RPC_URL        = %s\n' "$RPC_URL"
printf '     SOURCE_ACCOUNT = %s\n' "$SOURCE_ACCOUNT"
printf '     WASM_TARGET    = %s\n' "$WASM_TARGET"

# ─── 1. Toolchain preflight ───────────────────────────────────────────────────

info "Checking toolchain"
require_cmd cargo "Install Rust via https://rustup.rs/"
require_wasm_target
ok "Rust target '$WASM_TARGET' installed"
require_stellar_cli
ok "Stellar CLI: $($STELLAR --version 2>/dev/null | head -n1)"

# ─── 2. Local network ─────────────────────────────────────────────────────────

if [ "$START_NETWORK" -eq 1 ] && [ "$NETWORK" = "local" ]; then
  info "Starting local Stellar network (quickstart container)"
  require_cmd docker "Install Docker: https://docs.docker.com/get-docker/"
  docker info >/dev/null 2>&1 \
    || die "Docker is installed but the daemon is not reachable." \
           "Start Docker Desktop / the docker service, then re-run."

  if docker ps --format '{{.Names}}' | grep -q '^stellar$'; then
    ok "Container 'stellar' already running"
  else
    "$STELLAR" container start local || die \
      "Failed to start the local network container." \
      "Check for a port clash on 8000: docker ps -a; then 'make network-down'."
  fi

  info "Waiting for RPC to become healthy (this can take a few minutes)"
  for _ in $(seq 1 60); do
    body=$(curl -s --max-time 5 -X POST "$RPC_URL" \
      -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' 2>/dev/null) || true
    case "$(printf '%s' "${body:-}" | tr -d '[:space:]')" in
      *'"status":"healthy"'*) ok "RPC healthy"; break ;;
    esac
    printf '.'
    sleep 5
  done
  echo
else
  info "Skipping network startup"
fi

# ─── 3. Register the network with the CLI ─────────────────────────────────────

info "Registering network alias '$NETWORK'"
"$STELLAR" network add "$NETWORK" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE" \
  --overwrite 2>/dev/null \
  || "$STELLAR" network add "$NETWORK" \
       --rpc-url "$RPC_URL" \
       --network-passphrase "$NETWORK_PASSPHRASE" \
  || warn "Could not register network alias — it may already exist."
ok "Network '$NETWORK' configured"

# ─── 4. Identity ──────────────────────────────────────────────────────────────

info "Preparing deployer identity '$SOURCE_ACCOUNT'"
if "$STELLAR" keys address "$SOURCE_ACCOUNT" >/dev/null 2>&1; then
  ok "Identity already exists"
else
  "$STELLAR" keys generate --global "$SOURCE_ACCOUNT" --network "$NETWORK" \
    || die "Failed to generate identity '$SOURCE_ACCOUNT'." \
           "Check the CLI config dir is writable: stellar config-dir"
  ok "Identity created"
fi

ADDRESS="$($STELLAR keys address "$SOURCE_ACCOUNT")"

info "Funding $ADDRESS"
"$STELLAR" keys fund "$SOURCE_ACCOUNT" --network "$NETWORK" 2>/dev/null \
  || warn "Funding failed or the account is already funded (friendbot may rate-limit)."

echo
ok "Local environment ready."
printf '     Deployer: %s (%s)\n' "$SOURCE_ACCOUNT" "$ADDRESS"
printf '     Next:     ./scripts/deploy_local.sh\n'
