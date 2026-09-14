#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Build host-arch .deb + .rpm via cargo-deb / cargo-generate-rpm.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
OUT_DIR=${OUT_DIR:-dist}
mkdir -p "$OUT_DIR"

if [[ ! -d web/dashboard/dist ]]; then
  echo "error: build the dashboard first (cd web/dashboard && npm run build)" >&2
  exit 1
fi

if [[ ! -x target/release/zyvor-device-agent ]]; then
  echo "building release binary…"
  cargo build --release
fi

if ! command -v cargo-deb >/dev/null 2>&1; then
  cargo install cargo-deb --locked
fi
if ! command -v cargo-generate-rpm >/dev/null 2>&1; then
  cargo install cargo-generate-rpm --locked
fi

cargo deb -o "$OUT_DIR/"
cargo generate-rpm -o "$OUT_DIR/"

echo "packages in $OUT_DIR:"
ls -la "$OUT_DIR"/*.deb "$OUT_DIR"/*.rpm

HOST_ARCH=$(uname -m)
case "$HOST_ARCH" in
  aarch64|arm64) EXPECT='AArch64|ARM aarch64|ARM64|aarch64' ;;
  x86_64|amd64) EXPECT='x86-64|x86_64' ;;
  *) EXPECT='.' ;;
esac

DEB=$(ls -1 "$OUT_DIR"/*.deb | head -1)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
dpkg-deb -x "$DEB" "$TMP"
BIN="$TMP/usr/bin/zyvor-device-agent"
file "$BIN"
if ! file "$BIN" | grep -Eq "$EXPECT"; then
  echo "error: unexpected binary architecture in $DEB" >&2
  file "$BIN" >&2
  exit 1
fi
"$BIN" --version

RPM=$(ls -1 "$OUT_DIR"/*.rpm | head -1)
if command -v rpm >/dev/null 2>&1; then
  rpm -qp --qf '%{NAME}-%{VERSION}-%{RELEASE}.%{ARCH}\n' "$RPM"
fi

echo "PASS package-deb-rpm version=$VERSION host=$HOST_ARCH deb=$(basename "$DEB") rpm=$(basename "$RPM")"
