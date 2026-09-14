#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Start swtpm (TPM2 emulator) and export ZYVOR_SWTPM_TCTI for tests.
set -euo pipefail

STATE_DIR=${ZYVOR_SWTPM_STATE:-$(mktemp -d /tmp/zyvor-swtpm.XXXXXX)}
CTRL_PORT=${ZYVOR_SWTPM_CTRL_PORT:-2322}
DATA_PORT=${ZYVOR_SWTPM_DATA_PORT:-2321}
mkdir -p "$STATE_DIR"

if ! command -v swtpm >/dev/null; then
  echo "FAIL: swtpm not installed" >&2
  exit 1
fi

# Clean prior listeners on our ports if leftover from a previous job step.
if command -v fuser >/dev/null; then
  fuser -k "${DATA_PORT}/tcp" 2>/dev/null || true
  fuser -k "${CTRL_PORT}/tcp" 2>/dev/null || true
fi

swtpm socket --tpm2 \
  --tpmstate "dir=$STATE_DIR" \
  --ctrl "type=tcp,port=$CTRL_PORT" \
  --server "type=tcp,port=$DATA_PORT" \
  --flags not-need-init \
  >/tmp/zyvor-swtpm.log 2>&1 &
echo $! >/tmp/zyvor-swtpm.pid
sleep 0.5

if command -v swtpm_ioctl >/dev/null; then
  swtpm_ioctl --tcp ":$CTRL_PORT" -i || true
fi
if command -v tpm2_startup >/dev/null; then
  tpm2_startup --clear --tcti="swtpm:host=127.0.0.1,port=$DATA_PORT" || true
fi

export ZYVOR_SWTPM_TCTI="swtpm:host=127.0.0.1,port=$DATA_PORT"
echo "ZYVOR_SWTPM_TCTI=$ZYVOR_SWTPM_TCTI"
echo "swtpm pid=$(cat /tmp/zyvor-swtpm.pid) state=$STATE_DIR"
