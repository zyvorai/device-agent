#!/usr/bin/env bash
# Deploy zyvor-device-agent to a remote Linux host over SSH.
#
# Mirrors the ../fabric/scripts/deploy-remote.sh convention: nothing is
# built locally, sources are rsync'd to the remote host and `cargo build
# --release` (+ the dashboard's `npm run build`) runs there, then the
# binary is installed as a systemd service.
#
# Usage:
#   ./scripts/deploy-remote.sh USER@HOST [flags]
#   ./scripts/deploy-remote.sh HOST [flags]          # user defaults to $DEPLOY_USER or "sus"
#   ./scripts/deploy-remote.sh check USER@HOST        # health check only, no install
#
# Flags:
#   --quick            Skip OS/toolchain install steps (assume cargo/npm present)
#   --no-start         Install but don't enable/start the systemd service
#   --no-ui            Skip building/installing the dashboard
#   --bind ADDR        Address the service listens on (default: 127.0.0.1)
#   --auth-mode MODE   "none" (default) or "bearer" — bearer auto-generates a token
#                      on first deploy (see ZYVOR_DEVICE_AGENT_BEARER_TOKEN below).
#                      Only applied when seeding a brand-new config (same rule as
#                      --bind/--profile); redeploys never touch an existing config.
#   --uninstall        Remove the service/binary/dashboard, keep config and state
#   --dry-run          Print the steps without executing anything remote
#
# Env overrides: SSH_PORT, DEPLOY_DIR (remote checkout path),
#   ZYVOR_DEVICE_AGENT_BEARER_TOKEN (use this token instead of generating one),
#   FORCE_BEARER_TOKEN_RESET=1 (reissue the bearer token even if one already exists)
set -euo pipefail

APP=zyvor-device-agent
PORT=9188
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SSH_PORT="${SSH_PORT:-22}"
BIND=127.0.0.1
AUTH_MODE=none
MODE=install
QUICK=0
NO_START=0
NO_UI=0
DRY_RUN=0

log()  { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m!!\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31mERROR:\033[0m %s\n' "$*" >&2; exit 1; }

[[ -f "$REPO/Cargo.toml" ]] || die "run from the zyvor-device-agent repo (or its scripts/ dir)"

# --- argument parsing ---------------------------------------------------
if [[ "${1:-}" == "check" ]]; then
  MODE=check
  shift
fi

TARGET="${1:-}"
[[ -n "$TARGET" ]] || die "usage: $0 [check] USER@HOST [flags]"
shift || true

while [[ $# -gt 0 ]]; do
  case "$1" in
    --quick) QUICK=1 ;;
    --no-start) NO_START=1 ;;
    --no-ui) NO_UI=1 ;;
    --bind) BIND="$2"; shift ;;
    --auth-mode) AUTH_MODE="$2"; shift ;;
    --uninstall) MODE=uninstall ;;
    --dry-run) DRY_RUN=1 ;;
    *) die "unknown flag: $1" ;;
  esac
  shift
done

case "$AUTH_MODE" in
  none|bearer) ;;
  *) die "--auth-mode must be 'none' or 'bearer', got '$AUTH_MODE'" ;;
esac
case "$BIND" in
  127.0.0.1|localhost|::1) ;;
  *) [[ "$AUTH_MODE" == "none" ]] && die "refusing to bind $BIND with --auth-mode none (no auth) — pass --auth-mode bearer or keep --bind 127.0.0.1" ;;
esac

gen_token() { openssl rand -base64 24 | tr -d '/+=\n'; }
sha256_hex() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum | cut -d' ' -f1
  else shasum -a 256 | cut -d' ' -f1
  fi
}

if [[ "$TARGET" == *@* ]]; then
  REMOTE_USER="${TARGET%%@*}"
  REMOTE_HOST="${TARGET#*@}"
else
  REMOTE_USER="${DEPLOY_USER:-sus}"
  REMOTE_HOST="$TARGET"
fi
REMOTE="${REMOTE_USER}@${REMOTE_HOST}"

if [[ "$REMOTE_USER" == "root" ]]; then
  REMOTE_DIR="${DEPLOY_DIR:-/root/${APP}}"
else
  REMOTE_DIR="${DEPLOY_DIR:-/home/${REMOTE_USER}/${APP}}"
fi

SSH_OPTS=(-p "$SSH_PORT" -o StrictHostKeyChecking=accept-new -o ConnectTimeout=15
          -o ServerAliveInterval=15 -o ServerAliveCountMax=60 -o BatchMode=yes)
RSYNC_RSH="ssh ${SSH_OPTS[*]}"

ssh_r() { ssh "${SSH_OPTS[@]}" "$REMOTE" "$@"; }

# sudo prefix: passwordless if available, else fall back to a TTY session
# so an interactive sudo password prompt still works.
SUDO="sudo"
if [[ "$REMOTE_USER" != "root" ]]; then
  if ssh "${SSH_OPTS[@]}" "$REMOTE" 'sudo -n true' >/dev/null 2>&1; then
    SUDO="sudo -n"
  else
    warn "no passwordless sudo for $REMOTE_USER@$REMOTE_HOST — remote steps needing sudo will prompt interactively"
    SSH_OPTS+=(-tt)
  fi
fi

run() {
  log "$*"
  [[ "$DRY_RUN" == 1 ]] || ssh_r "$@"
}

# --- health / verification ----------------------------------------------
check_remote_health() {
  log "Checking $APP on $REMOTE_HOST"
  # Only known when this run just generated the token (or it's passed via env) — on a
  # --quick redeploy against an already-bearer-protected host with no token supplied,
  # INVENTORY/METRICS checks below are *expected* to fail closed; that's the auth
  # working correctly, not a deployment problem. Service-active + HEALTH_OK already
  # confirm the deploy itself succeeded.
  local auth_header=""
  local token="${ZYVOR_DEVICE_AGENT_BEARER_TOKEN:-${BEARER_TOKEN:-}}"
  [[ -n "$token" ]] && auth_header="-H 'Authorization: Bearer ${token}'"
  ssh_r "systemctl is-active ${APP}.service 2>&1 || true; systemctl is-enabled ${APP}.service 2>&1 || true"
  ssh_r "curl -sf --connect-timeout 3 http://127.0.0.1:${PORT}/api/v1/health && echo && echo HEALTH_OK || echo HEALTH_FAIL"
  ssh_r "curl -sf --connect-timeout 3 ${auth_header} http://127.0.0.1:${PORT}/api/v1/inventory >/dev/null && echo INVENTORY_OK || echo INVENTORY_FAIL"
  ssh_r "curl -sf --connect-timeout 3 ${auth_header} http://127.0.0.1:${PORT}/metrics >/dev/null && echo METRICS_OK || echo METRICS_FAIL"
}

if [[ "$MODE" == check ]]; then
  check_remote_health
  exit 0
fi

if [[ "$MODE" == uninstall ]]; then
  log "Uninstalling $APP from $REMOTE_HOST (config/state kept)"
  run "$SUDO systemctl disable --now ${APP}.service 2>/dev/null || true"
  run "$SUDO rm -f /etc/systemd/system/${APP}.service"
  run "$SUDO systemctl daemon-reload"
  run "$SUDO rm -f /usr/bin/${APP}"
  run "$SUDO rm -rf /usr/share/${APP}"
  log "Kept /etc/zyvor/device-agent* and /var/lib/${APP} (config + state)"
  exit 0
fi

# --- 1. sync sources ------------------------------------------------------
log "Syncing sources to ${REMOTE}:${REMOTE_DIR}"
if [[ "$DRY_RUN" == 0 ]]; then
  ssh_r "mkdir -p '$REMOTE_DIR'"
  rsync -az --delete -e "$RSYNC_RSH" \
    --exclude target/ --exclude .git/ \
    --exclude web/dashboard/node_modules/ --exclude web/dashboard/dist/ \
    --exclude dist/ \
    "$REPO/" "$REMOTE:$REMOTE_DIR/"
fi

# --- 2. fix ownership -------------------------------------------------------
run "$SUDO chown -R \$(id -un):\$(id -gn) '$REMOTE_DIR'"

# --- 3. toolchains (unless --quick) -----------------------------------------
if [[ "$QUICK" == 0 ]]; then
  log "Ensuring build toolchains are present"
  run "command -v cargo >/dev/null || (curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y)"
  run "$SUDO apt-get update -qq && $SUDO apt-get install -y -qq build-essential pkg-config"
  if [[ "$NO_UI" == 0 ]]; then
    run "command -v npm >/dev/null || ($SUDO apt-get install -y -qq nodejs npm)"
  fi
fi

# --- 4. build on remote ------------------------------------------------------
log "Building $APP release binary on remote"
run "source \$HOME/.cargo/env 2>/dev/null; cd '$REMOTE_DIR' && cargo build --release"

if [[ "$NO_UI" == 0 ]]; then
  log "Building dashboard on remote"
  run "cd '$REMOTE_DIR/web/dashboard' && npm install --no-audit --no-fund && npm run build"
fi

# --- 5. install binary, config, profiles, dashboard --------------------------
log "Installing binary and support files"
run "$SUDO install -m755 '$REMOTE_DIR/target/release/${APP}' /usr/bin/${APP}"
run "$SUDO mkdir -p /var/lib/${APP} /run/${APP} /etc/zyvor/device-agent/profiles /etc/zyvor/device-agent/plugins.d /usr/lib/${APP}/plugins"
run "$SUDO cp '$REMOTE_DIR'/profiles/*.toml /etc/zyvor/device-agent/profiles/"
run "$SUDO install -m755 '$REMOTE_DIR/examples/i2c_temperature.py' /usr/lib/${APP}/plugins/i2c_temperature.py"
run "$SUDO install -m644 '$REMOTE_DIR/examples/plugins.d/i2c-temperature.json' /etc/zyvor/device-agent/plugins.d/i2c-temperature.json"

run "if [[ ! -f /etc/zyvor/device-agent.toml ]]; then \
  $SUDO cp '$REMOTE_DIR/config/device-agent.example.toml' /etc/zyvor/device-agent.toml && \
  $SUDO sed -i 's|^listen = .*|listen = \"${BIND}:${PORT}\"|' /etc/zyvor/device-agent.toml && \
  $SUDO sed -i 's|^profile = .*|profile = \"generic-linux-arm64\"|' /etc/zyvor/device-agent.toml && \
  $SUDO sed -i 's|^profile_directory = .*|profile_directory = \"/etc/zyvor/device-agent/profiles\"|' /etc/zyvor/device-agent.toml && \
  $SUDO sed -i '/^\[auth\]\$/,/^\[auth\.bearer\]\$/{s|^mode = .*|mode = \"${AUTH_MODE}\"|}' /etc/zyvor/device-agent.toml; \
else echo 'config already present, leaving untouched'; fi"

# --- 5b. bearer token (only when --auth-mode bearer) -------------------------
if [[ "$AUTH_MODE" == "bearer" && "$DRY_RUN" == 0 ]]; then
  log "Configuring bearer auth"
  run "$SUDO mkdir -p /etc/zyvor/device-agent/auth"
  HASH_EXISTS="$(ssh_r "test -f /etc/zyvor/device-agent/auth/bearer.sha256 && echo yes || echo no")"
  if [[ "$HASH_EXISTS" == "no" || -n "${FORCE_BEARER_TOKEN_RESET:-}" ]]; then
    BEARER_TOKEN="${ZYVOR_DEVICE_AGENT_BEARER_TOKEN:-$(gen_token)}"
    BEARER_HASH="$(printf '%s' "$BEARER_TOKEN" | sha256_hex)"
    log "Writing bearer token hash"
    printf '%s\n' "$BEARER_HASH" | ssh_r "$SUDO tee /etc/zyvor/device-agent/auth/bearer.sha256 >/dev/null && $SUDO chmod 600 /etc/zyvor/device-agent/auth/bearer.sha256"
    BEARER_TOKEN_PRINTED=1
  else
    log "Bearer token already configured on remote (set FORCE_BEARER_TOKEN_RESET=1 to reissue)"
    BEARER_TOKEN_PRINTED=0
  fi
fi

if [[ "$NO_UI" == 0 ]]; then
  run "$SUDO rm -rf /usr/share/${APP}/dashboard && $SUDO mkdir -p /usr/share/${APP}/dashboard && $SUDO cp -r '$REMOTE_DIR/web/dashboard/dist/.' /usr/share/${APP}/dashboard/"
fi

# --- 6. systemd unit ----------------------------------------------------------
log "Installing systemd unit"
run "$SUDO install -m644 '$REMOTE_DIR/packaging/systemd/${APP}.service' /etc/systemd/system/${APP}.service"
run "$SUDO systemctl daemon-reload"

if [[ "$NO_START" == 0 ]]; then
  # enable --now only starts a not-yet-running unit; restart explicitly so a
  # redeploy actually picks up the newly installed binary.
  run "$SUDO systemctl enable ${APP}.service"
  run "$SUDO systemctl restart ${APP}.service"
  run "sleep 2; $SUDO systemctl is-active ${APP}.service"
fi

# --- 7. health check -----------------------------------------------------------
if [[ "$NO_START" == 0 && "$DRY_RUN" == 0 ]]; then
  check_remote_health
fi

# --- 8. record last deploy ------------------------------------------------------
if [[ "$DRY_RUN" == 0 ]]; then
  VERSION="$(git -C "$REPO" describe --tags --always --dirty 2>/dev/null || echo unknown)"
  COMMIT="$(git -C "$REPO" rev-parse --short HEAD 2>/dev/null || echo unknown)"
  cat > "$REPO/.deploy-last" <<EOF
# Auto-generated by scripts/deploy-remote.sh (no passwords)
HOST=${REMOTE_HOST}
USER=${REMOTE_USER}
MODE=$([[ "$QUICK" == 1 ]] && echo quick || echo full)
UPDATED=$(date -u +%Y-%m-%dT%H:%M:%SZ)
VERSION=${VERSION}
COMMIT=${COMMIT}
EOF
  log "Deployed ${APP} to ${REMOTE}. Recorded in .deploy-last"
  log "Service listens on ${BIND}:${PORT} (not exposed beyond localhost by default)"
  log "Tail logs: ssh ${REMOTE} 'journalctl -u ${APP} -f'"
  if [[ "${BEARER_TOKEN_PRINTED:-0}" == "1" ]]; then
    log "Bearer token (generated · save this, it is not stored on the remote): ${BEARER_TOKEN}"
    log "Retrieve the hash later with: ssh ${REMOTE} '${SUDO} cat /etc/zyvor/device-agent/auth/bearer.sha256'"
  fi
fi
