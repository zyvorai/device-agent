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

ensure_vcan_module() {
  if lsmod | grep -q '^vcan '; then
    return 0
  fi
  if sudo modprobe vcan 2>/dev/null; then
    return 0
  fi
  # GitHub-hosted Azure kernels often omit vcan unless modules-extra is installed.
  if command -v apt-get >/dev/null; then
    echo "vcan module missing; installing linux-modules-extra-$(uname -r)" >&2
    sudo apt-get update -qq
    sudo apt-get install -y "linux-modules-extra-$(uname -r)" || true
  fi
  if ! sudo modprobe vcan; then
    echo "FAIL: could not load vcan (kernel=$(uname -r))" >&2
    exit 1
  fi
}

ensure_vcan_module
if ! ip link show "$IFACE" >/dev/null 2>&1; then
  sudo ip link add dev "$IFACE" type vcan
fi
sudo ip link set up "$IFACE"
ip -details link show "$IFACE"
echo "vcan ready: $IFACE"
