#!/usr/bin/env bash
# Shared helpers for the Aegis deployment scripts.
# Sourced by scripts/*.sh — not meant to be executed directly.

# ─── Output helpers ───────────────────────────────────────────────────────────

if [ -t 1 ]; then
  C_RESET=$'\033[0m'; C_RED=$'\033[31m'; C_GREEN=$'\033[32m'
  C_YELLOW=$'\033[33m'; C_BLUE=$'\033[36m'
else
  C_RESET=""; C_RED=""; C_GREEN=""; C_YELLOW=""; C_BLUE=""
fi

info()  { printf '%s==>%s %s\n' "$C_BLUE"   "$C_RESET" "$*"; }
ok()    { printf '%s  ok%s %s\n' "$C_GREEN"  "$C_RESET" "$*"; }
warn()  { printf '%swarn%s %s\n' "$C_YELLOW" "$C_RESET" "$*" >&2; }
err()   { printf '%s ERR%s %s\n' "$C_RED"    "$C_RESET" "$*" >&2; }

# Print an error with a remediation hint, then exit non-zero.
die() {
  err "$1"
  if [ -n "${2:-}" ]; then
    printf '     %sfix:%s %s\n' "$C_YELLOW" "$C_RESET" "$2" >&2
  fi
  exit 1
}

# ─── Repo root + configuration ────────────────────────────────────────────────

# Absolute path to the repository root, regardless of the caller's cwd.
repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

REPO_ROOT="$(repo_root)"

# Load .env if present. Values already exported in the environment win, so
# `NETWORK=testnet ./scripts/deploy_local.sh` overrides the file.
load_env() {
  local env_file="$REPO_ROOT/.env"
  if [ -f "$env_file" ]; then
    info "Loading configuration from .env"
    # shellcheck disable=SC1090
    while IFS= read -r line || [ -n "$line" ]; do
      case "$line" in
        ''|'#'*) continue ;;
      esac
      local key="${line%%=*}"
      local value="${line#*=}"
      # Strip surrounding quotes if present.
      value="${value%\"}"; value="${value#\"}"
      # Only set if not already defined in the environment.
      if [ -z "${!key:-}" ]; then
        export "$key=$value"
      fi
    done < "$env_file"
  else
    warn "No .env found — using built-in defaults. Run: cp .env.example .env"
  fi
}

# Apply defaults for anything still unset after load_env.
apply_defaults() {
  export WASM_TARGET="${WASM_TARGET:-wasm32v1-none}"
  export CONTRACT_NAME="${CONTRACT_NAME:-aegis_contracts}"
  export STELLAR="${STELLAR:-stellar}"
  export NETWORK="${NETWORK:-local}"
  export RPC_URL="${RPC_URL:-http://localhost:8000/rpc}"
  export NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
  export SOURCE_ACCOUNT="${SOURCE_ACCOUNT:-aegis-admin}"
  export AUTO_INITIALIZE="${AUTO_INITIALIZE:-1}"
  export WASM_PATH="$REPO_ROOT/target/$WASM_TARGET/release/$CONTRACT_NAME.wasm"
  export DEPLOY_OUT="$REPO_ROOT/.aegis-deploy"
}

# ─── Preflight checks ─────────────────────────────────────────────────────────

require_cmd() {
  command -v "$1" >/dev/null 2>&1 \
    || die "Required command '$1' is not installed or not on PATH." "${2:-Install '$1' and re-run.}"
}

require_stellar_cli() {
  command -v "$STELLAR" >/dev/null 2>&1 || die \
    "Stellar CLI ('$STELLAR') not found on PATH." \
    "cargo install --locked stellar-cli   (or export STELLAR=soroban for the legacy binary)"
}

require_wasm_target() {
  require_cmd rustup "Install Rust via https://rustup.rs/"
  rustup target list --installed 2>/dev/null | grep -qx "$WASM_TARGET" || die \
    "Rust target '$WASM_TARGET' is not installed." \
    "rustup target add $WASM_TARGET"
}

# Verify the RPC endpoint answers a getHealth call before we try to deploy.
require_rpc_healthy() {
  require_cmd curl
  local body
  body=$(curl -s --max-time 10 -X POST "$RPC_URL" \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' 2>/dev/null) || true

  if [ -z "$body" ]; then
    die "No response from RPC at $RPC_URL" \
        "Start the local network first: make network-up   (or scripts/setup_local.sh)"
  fi

  # Normalize: drop all whitespace so the match is insensitive to how the
  # server formats its JSON.
  local compact
  compact="$(printf '%s' "$body" | tr -d '[:space:]')"

  case "$compact" in
    *'"status":"healthy"'*) ok "RPC healthy at $RPC_URL" ;;
    *) die "RPC at $RPC_URL is reachable but not healthy: $body" \
           "Wait for the quickstart container to finish syncing, then retry." ;;
  esac
}
