#!/usr/bin/env bash
# Post-deploy verification for zyvor-device-agent.
#
# Usage: ./scripts/verify-deployment.sh [HOST] [USER] [PORT]
#   HOST defaults to "localhost" (checks run locally, no SSH)
#   USER defaults to "sus"
#   PORT defaults to 9188
#
# Env: ZYVOR_DEVICE_AGENT_BEARER_TOKEN — pass when auth.mode = "bearer" on the
#   target, so the inventory/metrics checks authenticate instead of getting 401.
#
# All checks below use plain http://. If the target has `server.tls.enabled = true`
# (see README's "TLS" section), the three curl-based checks fail closed - that's the
# daemon correctly speaking TLS-only, not a broken deployment; verify by hand with
# curl -sSk https://127.0.0.1:PORT/api/v1/health instead.
set -uo pipefail

APP=zyvor-device-agent
HOST="${1:-localhost}"
REMOTE_USER="${2:-sus}"
PORT="${3:-9188}"
AUTH_HEADER=""
[[ -n "${ZYVOR_DEVICE_AGENT_BEARER_TOKEN:-}" ]] && AUTH_HEADER="-H 'Authorization: Bearer ${ZYVOR_DEVICE_AGENT_BEARER_TOKEN}'"

CHECKS=0
PASSED=0

pass() { CHECKS=$((CHECKS+1)); PASSED=$((PASSED+1)); printf '  \033[1;32mPASS\033[0m  %s\n' "$1"; }
fail() { CHECKS=$((CHECKS+1)); printf '  \033[1;31mFAIL\033[0m  %s\n' "$1"; }

if [[ "$HOST" == "localhost" || "$HOST" == "127.0.0.1" ]]; then
  run() { bash -c "$1"; }
else
  run() { ssh -o ConnectTimeout=10 -o BatchMode=yes "${REMOTE_USER}@${HOST}" "$1"; }
fi

echo "Verifying ${APP} on ${HOST}"
echo

if run "systemctl is-active --quiet ${APP}.service" 2>/dev/null; then
  pass "systemd service is active"
else
  fail "systemd service is not active"
fi

if run "command -v ${APP} >/dev/null"; then
  pass "binary on PATH"
else
  fail "binary not found on PATH"
fi

if run "test -f /etc/zyvor/device-agent.toml"; then
  pass "config file present (/etc/zyvor/device-agent.toml)"
else
  fail "config file missing"
fi

if run "curl -sf --connect-timeout 3 http://127.0.0.1:${PORT}/api/v1/health >/dev/null"; then
  pass "GET /api/v1/health -> 200"
else
  fail "GET /api/v1/health failed"
fi

if run "curl -sf --connect-timeout 3 ${AUTH_HEADER} http://127.0.0.1:${PORT}/api/v1/inventory >/dev/null"; then
  pass "GET /api/v1/inventory -> 200"
else
  fail "GET /api/v1/inventory failed (pass ZYVOR_DEVICE_AGENT_BEARER_TOKEN if auth.mode = bearer)"
fi

if run "curl -sf --connect-timeout 3 ${AUTH_HEADER} http://127.0.0.1:${PORT}/metrics >/dev/null"; then
  pass "GET /metrics -> 200"
else
  fail "GET /metrics failed (pass ZYVOR_DEVICE_AGENT_BEARER_TOKEN if auth.mode = bearer)"
fi

echo
echo "${PASSED}/${CHECKS} checks passed"

if [[ "$PASSED" -ne "$CHECKS" ]]; then
  echo
  echo "Debug with:"
  echo "  ssh ${REMOTE_USER}@${HOST} 'systemctl status ${APP} --no-pager; journalctl -u ${APP} -n 20 --no-pager'"
  exit 1
fi
