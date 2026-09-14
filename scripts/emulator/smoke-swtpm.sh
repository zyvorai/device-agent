#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Smoke: generate/persist/sign a TPM-backed identity against swtpm.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
STRICT=${DA_EMULATOR_STRICT:-1}

skip_or_fail() {
  echo "$1" >&2
  if [[ "$STRICT" == "1" ]]; then
    exit 1
  fi
  echo "SKIP: $1"
  exit 0
}

if ! command -v swtpm >/dev/null; then
  skip_or_fail "swtpm not installed"
fi
if ! pkg-config --exists tss2-esys 2>/dev/null; then
  skip_or_fail "libtss2-dev / tss2-esys not available"
fi

# shellcheck disable=SC1091
source "$ROOT/scripts/emulator/setup-swtpm.sh" || skip_or_fail "swtpm setup failed"
trap 'kill "$(cat /tmp/zyvor-swtpm.pid 2>/dev/null)" 2>/dev/null || true' EXIT

export ZYVOR_SWTPM_TCTI
cargo test --features tpm2 --manifest-path "$ROOT/Cargo.toml" \
  identity::tpm::live_swtpm_tests -- --nocapture \
  || skip_or_fail "swtpm identity live tests failed"

echo "PASS emulator-swtpm ($ZYVOR_SWTPM_TCTI)"
