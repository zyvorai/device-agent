#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Run all emulator smokes that are available on this host.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
STRICT=${DA_EMULATOR_STRICT:-0}
export DA_EMULATOR_STRICT="$STRICT"

rc=0
"$ROOT/scripts/emulator/smoke-vcan.sh" || rc=$?
"$ROOT/scripts/emulator/smoke-swtpm.sh" || rc=$?
"$ROOT/scripts/emulator/smoke-v4l2.sh" || rc=$?
exit "$rc"
