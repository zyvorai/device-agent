#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Bring up a virtual CAN interface for emulator smoke tests.
set -euo pipefail

IFACE=${ZYVOR_VCAN_IFACE:-vcan0}

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "SKIP: vcan requires Linux" >&2
  exit 2
fi

if ! command -v ip >/dev/null; then
  echo "FAIL: iproute2 (ip) required" >&2
  exit 1
fi

sudo modprobe vcan
if ! ip link show "$IFACE" >/dev/null 2>&1; then
  sudo ip link add dev "$IFACE" type vcan
fi
sudo ip link set up "$IFACE"
ip -details link show "$IFACE"
echo "vcan ready: $IFACE"
