#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# CI substitute for Minewing physical HIL when the lab has no board.
#
# Starts Device Agent with vcan + minewing profile label, runs the HIL
# harness as DA_HIL_ENV=ci-emulator (never claimable / never signs).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
WORK=$(mktemp -d)
PORT=${DA_HIL_CI_PORT:-18788}
STRICT=${DA_EMULATOR_STRICT:-1}
export DA_EMULATOR_STRICT="$STRICT"

cleanup() {
  kill "${AGENT_PID:-0}" 2>/dev/null || true
  wait "${AGENT_PID:-0}" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

IFACE=${ZYVOR_VCAN_IFACE:-vcan0}
"$ROOT/scripts/emulator/setup-vcan.sh" || {
  rc=$?
  if [[ $rc -eq 2 ]]; then
    echo "SKIP: non-Linux host (HIL CI emulator requires Linux runner)"
    exit 0
  fi
  echo "vcan setup failed rc=$rc" >&2
  exit 1
}

command -v curl >/dev/null || { echo "curl required" >&2; exit 1; }
cargo build --release -q --manifest-path "$ROOT/Cargo.toml"

CFG="$WORK/device-agent.toml"
STATE="$WORK/state"
mkdir -p "$STATE"
# Profile name is minewing for doctor evidence; buses stay emulator-grade.
cat >"$CFG" <<EOF
[device]
serial = "ci-emulator-hil"
profile = "minewing-gw1-r1"
telemetry_interval_seconds = 30
inventory_refresh_seconds = 30

[server]
listen = "127.0.0.1:$PORT"

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

for _ in $(seq 1 60); do
  if curl -fsS "http://127.0.0.1:$PORT/api/v1/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.25
done
curl -fsS "http://127.0.0.1:$PORT/api/v1/health" >/dev/null

# Inject one CAN frame so inventory/can paths have something to show.
if command -v cansend >/dev/null; then
  cansend "$IFACE" 123#DEADBEEF || true
fi

export DA_HIL_BASE="http://127.0.0.1:$PORT"
export DA_HIL_ENV=ci-emulator
export DA_HIL_PROFILE=minewing-gw1-r1
export DA_HIL_STRICT=0
export DA_HIL_SIGN=0
"$ROOT/scripts/hil/run-minewing-hil.sh"

# Copy latest HIL stamp into ci/ evidence and assert not claimable
python3 - "$ROOT" <<'PY'
import json, pathlib, shutil, sys
root = pathlib.Path(sys.argv[1])
hil = root / "evidence/qualification/hil"
stamps = sorted(hil.glob("*/results.json"), reverse=True) if hil.is_dir() else []
if not stamps:
    sys.exit("no HIL results")
data = json.loads(stamps[0].read_text())
if data.get("minewing_claimable"):
    sys.exit("ci-emulator must never be minewing_claimable")
ci = root / "evidence/qualification/ci" / stamps[0].parent.name
ci.mkdir(parents=True, exist_ok=True)
for p in stamps[0].parent.iterdir():
    if p.is_file():
        shutil.copy2(p, ci / p.name)
summary = {
    "product": "zyvor-device-agent",
    "environment": "ci-emulator",
    "minewing_claimable": False,
    "hil_stamp": stamps[0].parent.name,
    "counts": data.get("counts"),
    "note": "GitHub CI emulator HIL — does not close physical Minewing rows",
}
(ci / "ci-wrapper.json").write_text(json.dumps(summary, indent=2) + "\n")
print(f"ci emulator HIL ok stamp={stamps[0].parent.name} claimable=false")
PY

echo "PASS: CI emulator HIL (non-claimable)"
