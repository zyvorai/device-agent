#!/usr/bin/env bash
# Post-deploy verification for zyvor-device-agent.
#
# Usage: ./scripts/verify-deployment.sh [HOST] [USER] [PORT]
#   HOST defaults to "localhost" (checks run locally, no SSH)
#   USER defaults to "sus"
#   PORT defaults to 9188
#
# Env:
#   ZYVOR_DEVICE_AGENT_BEARER_TOKEN — when auth.mode = "bearer"
#   ZYVOR_DEVICE_AGENT_TLS=1 — force HTTPS (-k for self-signed)
#   ZYVOR_DEVICE_AGENT_CA=/path/ca.pem — HTTPS with CA verify (implies TLS)
#
# HTTP is tried first unless TLS is forced. If HTTP fails, HTTPS with -k is
# tried automatically so TLS-only deployments do not false-FAIL.
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

curl_api() {
  # $1 = path including leading slash
  local path="$1"
  local auth="${AUTH_HEADER}"
  local force_tls=0
  local ca_args=""
  if [[ -n "${ZYVOR_DEVICE_AGENT_CA:-}" ]]; then
    force_tls=1
    ca_args="--cacert '${ZYVOR_DEVICE_AGENT_CA}'"
  elif [[ "${ZYVOR_DEVICE_AGENT_TLS:-}" == "1" || "${ZYVOR_DEVICE_AGENT_TLS:-}" == "true" ]]; then
    force_tls=1
  fi
  if [[ "$force_tls" -eq 1 ]]; then
    if [[ -n "$ca_args" ]]; then
      run "curl -sf --connect-timeout 3 ${ca_args} ${auth} https://127.0.0.1:${PORT}${path} >/dev/null"
    else
      run "curl -sSk --connect-timeout 3 ${auth} https://127.0.0.1:${PORT}${path} >/dev/null"
    fi
    return $?
  fi
  if run "curl -sf --connect-timeout 3 ${auth} http://127.0.0.1:${PORT}${path} >/dev/null"; then
    return 0
  fi
  run "curl -sSk --connect-timeout 3 ${auth} https://127.0.0.1:${PORT}${path} >/dev/null"
}

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

if curl_api "/api/v1/health"; then
  pass "GET /api/v1/health -> 200 (http or https)"
else
  fail "GET /api/v1/health failed"
fi

if curl_api "/api/v1/inventory"; then
  pass "GET /api/v1/inventory -> 200"
else
  fail "GET /api/v1/inventory failed (pass ZYVOR_DEVICE_AGENT_BEARER_TOKEN if auth.mode = bearer)"
fi

if curl_api "/metrics"; then
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
  echo "TLS tip: ZYVOR_DEVICE_AGENT_TLS=1 or ZYVOR_DEVICE_AGENT_CA=/path/ca.pem"
  exit 1
fi
