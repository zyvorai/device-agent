#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

if [[ -z "${VERSION:-}" ]]; then
  VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
fi
ARCH="${ARCH:-$(uname -m)}"
case "$ARCH" in
  aarch64) ARCH=linux-arm64 ;;
  x86_64) ARCH=linux-amd64 ;;
esac

BIN=target/release/zyvor-device-agent
if [[ ! -x "$BIN" ]]; then
  # Prefer an explicit cross-target binary when packaging from amd64 for arm64.
  if [[ "$ARCH" == "linux-arm64" && -x target/aarch64-unknown-linux-gnu/release/zyvor-device-agent ]]; then
    BIN=target/aarch64-unknown-linux-gnu/release/zyvor-device-agent
  else
    echo "error: missing $BIN — build the release binary first" >&2
    exit 1
  fi
fi

OUT="dist/zyvor-device-agent-${VERSION}-${ARCH}"
rm -rf "$OUT"
mkdir -p "$OUT/bin" "$OUT/config" "$OUT/systemd" "$OUT/dashboard" "$OUT/profiles" "$OUT/plugins" "$OUT/plugins.d"
cp "$BIN" "$OUT/bin/"
cp config/device-agent.example.toml "$OUT/config/device-agent.toml"
cp packaging/systemd/zyvor-device-agent.service "$OUT/systemd/"
cp profiles/*.toml "$OUT/profiles/"
cp examples/i2c_temperature.py "$OUT/plugins/"
cp examples/plugins.d/i2c-temperature.json "$OUT/plugins.d/"
cp -r web/dashboard/dist/. "$OUT/dashboard/"
tar -C dist -czf "${OUT}.tar.gz" "$(basename "$OUT")"
echo "Created ${OUT}.tar.gz"
