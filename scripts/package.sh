#!/usr/bin/env bash
set -euo pipefail
VERSION="${VERSION:-0.1.0}"
ARCH="${ARCH:-$(uname -m)}"
OUT="dist/zyvor-device-agent-${VERSION}-${ARCH}"
rm -rf "$OUT"
mkdir -p "$OUT/bin" "$OUT/config" "$OUT/systemd" "$OUT/dashboard" "$OUT/profiles"
cp target/release/zyvor-device-agent "$OUT/bin/"
cp config/device-agent.example.toml "$OUT/config/device-agent.toml"
cp packaging/systemd/zyvor-device-agent.service "$OUT/systemd/"
cp profiles/*.toml "$OUT/profiles/"
cp -r web/dashboard/dist/. "$OUT/dashboard/"
tar -C dist -czf "${OUT}.tar.gz" "$(basename "$OUT")"
echo "Created ${OUT}.tar.gz"
