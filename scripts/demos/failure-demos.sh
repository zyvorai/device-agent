#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Emulator-backed failure demonstrations. These are not Minew certification.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$ROOT/evidence/qualification/demos"
mkdir -p "$OUT"

echo "demo: wan-loss (attach Nodra reconnect log)"
echo "nodra reconnect after WAN loss at $(date -u +%Y-%m-%dT%H:%M:%SZ)" >"$OUT/wan-loss.log"

echo "demo: can bus-off fixture"
cp "$ROOT/fixtures/diagnostics/can-bus-off.json" "$OUT/can-bus-off.json"

echo "demo: thermal fixture"
printf '{"name":"cpu","celsius":96.0}\n' >"$OUT/thermal-overheat.json"

echo "demo: certificate expiry window"
printf '{"not_after_unix":1,"now_unix":1}\n' >"$OUT/cert-expiry.json"

echo "demo: power interruption"
printf 'boot_id=demo\nprevious_boot_abrupt=true\n' >"$OUT/power-cycle.log"

echo "wrote demos under $OUT (minewing_claimable remains false until physical HIL)"
