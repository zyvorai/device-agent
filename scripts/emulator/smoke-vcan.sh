#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Smoke: capture one frame from vcan via the running agent API.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
IFACE=${ZYVOR_VCAN_IFACE:-vcan0}
STRICT=${DA_EMULATOR_STRICT:-1}
WORK=$(mktemp -d)
trap 'kill ${AGENT_PID:-0} 2>/dev/null || true; rm -rf "$WORK"' EXIT

skip_or_fail() {
  echo "$1" >&2
  if [[ "$STRICT" == "1" ]]; then
    exit 1
  fi
  echo "SKIP: $1"
  exit 0
}

"$ROOT/scripts/emulator/setup-vcan.sh" || {
  rc=$?
  if [[ $rc -eq 2 ]]; then
    echo "SKIP: non-Linux host"
    exit 0
  fi
  skip_or_fail "vcan setup failed (rc=$rc)"
}

command -v cansend >/dev/null || skip_or_fail "can-utils (cansend) not installed"
command -v curl >/dev/null || skip_or_fail "curl required"

cargo build --release -q --manifest-path "$ROOT/Cargo.toml"

CFG="$WORK/device-agent.toml"
STATE="$WORK/state"
mkdir -p "$STATE"
cat >"$CFG" <<EOF
[device]
serial = "emulator-vcan"
profile = "generic-linux-arm64"
telemetry_interval_seconds = 30
inventory_refresh_seconds = 30

[server]
listen = "127.0.0.1:18765"

[auth]
mode = "none"

[industrial.can_capture]
enabled = true
interfaces = ["$IFACE"]
history_limit = 64
max_frames_per_second = 50
publish_to_nodra = false

[nodra]
enabled = false
EOF

"$ROOT/target/release/zyvor-device-agent" --config "$CFG" serve >"$WORK/agent.log" 2>&1 &
AGENT_PID=$!

for _ in $(seq 1 40); do
  if curl -fsS "http://127.0.0.1:18765/api/v1/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.25
done
curl -fsS "http://127.0.0.1:18765/api/v1/health" >/dev/null

# classic CAN id 0x123, payload deadbeef
cansend "$IFACE" 123#DEADBEEF

ok=0
for _ in $(seq 1 40); do
  body=$(curl -fsS "http://127.0.0.1:18765/api/v1/can/frames/recent" || true)
  if echo "$body" | grep -qi 'DEADBEEF\|deadbeef'; then
    ok=1
    break
  fi
  if echo "$body" | grep -q '"can_id"[[:space:]]*:[[:space:]]*291'; then
    ok=1
    break
  fi
  sleep 0.25
done

cap=$(curl -fsS "http://127.0.0.1:18765/api/v1/can/capture" || true)
echo "capture status: $cap"
if [[ "$ok" -ne 1 ]]; then
  echo "agent log:" >&2
  tail -n 80 "$WORK/agent.log" >&2 || true
  skip_or_fail "did not observe CAN frame on /api/v1/can/frames/recent"
fi

echo "PASS emulator-vcan ($IFACE)"
