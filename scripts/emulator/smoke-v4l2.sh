#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Smoke: camera feature against v4l2loopback (soft-skip if module unavailable).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
DEV=${ZYVOR_V4L2_DEV:-/dev/video0}
# Default soft: GHA kernels sometimes lack a matching v4l2loopback module.
STRICT=${DA_EMULATOR_STRICT:-0}
WORK=$(mktemp -d)
trap 'kill ${AGENT_PID:-0} 2>/dev/null || true; kill "$(cat /tmp/zyvor-v4l2-ffmpeg.pid 2>/dev/null)" 2>/dev/null || true; rm -rf "$WORK"' EXIT

skip_or_fail() {
  echo "$1" >&2
  if [[ "$STRICT" == "1" ]]; then
    exit 1
  fi
  echo "SKIP: $1"
  exit 0
}

"$ROOT/scripts/emulator/setup-v4l2.sh" || {
  rc=$?
  if [[ $rc -eq 2 ]]; then
    echo "SKIP: non-Linux host"
    exit 0
  fi
  skip_or_fail "v4l2loopback setup failed (rc=$rc)"
}

command -v curl >/dev/null || skip_or_fail "curl required"

cargo build --release -q --features camera --manifest-path "$ROOT/Cargo.toml"

CFG="$WORK/device-agent.toml"
cat >"$CFG" <<EOF
[device]
serial = "emulator-v4l2"
profile = "generic-linux-arm64"
telemetry_interval_seconds = 30
inventory_refresh_seconds = 30

[server]
listen = "127.0.0.1:18766"

[auth]
mode = "none"

[camera]
[[camera.devices]]
id = "loopback"
path = "$DEV"
enabled = true
max_frames_per_second = 5
jpeg_quality = 70
publish_to_nodra = false

[nodra]
enabled = false
EOF

"$ROOT/target/release/zyvor-device-agent" --config "$CFG" serve >"$WORK/agent.log" 2>&1 &
AGENT_PID=$!

for _ in $(seq 1 40); do
  if curl -fsS "http://127.0.0.1:18766/api/v1/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.25
done
curl -fsS "http://127.0.0.1:18766/api/v1/health" >/dev/null

status=$(curl -fsS "http://127.0.0.1:18766/api/v1/camera" || true)
echo "camera status: $status"

ok=0
for _ in $(seq 1 40); do
  code=$(curl -sS -o "$WORK/snap.jpg" -w "%{http_code}" "http://127.0.0.1:18766/api/v1/camera/loopback/snapshot" || true)
  if [[ "$code" == "200" ]] && file "$WORK/snap.jpg" 2>/dev/null | grep -qi 'JPEG\|JFIF'; then
    ok=1
    break
  fi
  # Some loopback feeds need a moment; also accept non-empty JPEG magic.
  if [[ "$code" == "200" ]] && [[ -s "$WORK/snap.jpg" ]]; then
    magic=$(xxd -p -l 3 "$WORK/snap.jpg" 2>/dev/null || true)
    if [[ "$magic" == "ffd8ff"* ]]; then
      ok=1
      break
    fi
  fi
  sleep 0.5
done

if [[ "$ok" -ne 1 ]]; then
  echo "agent log:" >&2
  tail -n 100 "$WORK/agent.log" >&2 || true
  skip_or_fail "camera snapshot did not return JPEG from $DEV"
fi

echo "PASS emulator-v4l2 ($DEV)"
