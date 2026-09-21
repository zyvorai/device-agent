#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Zero-touch / local install for Zyvor Device Agent.
#
#   curl -fsSL https://get.zyvor.dev/device-agent | sudo sh -s -- --enrollment-token TOKEN
#
# The get.zyvor.dev host is site hosting. This script is the installer those
# bytes should serve.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_SOURCE="${BIN_SOURCE:-$ROOT/target/release/zyvor-device-agent}"
ENROLLMENT_TOKEN=""
ENROLLMENT_URL=""
SKIP_COMMISSION=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --enrollment-token) ENROLLMENT_TOKEN="$2"; shift 2 ;;
    --enrollment-url) ENROLLMENT_URL="$2"; shift 2 ;;
    --skip-commission) SKIP_COMMISSION=1; shift ;;
    --bin) BIN_SOURCE="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ ! -x "$BIN_SOURCE" ]]; then
  echo "missing executable: $BIN_SOURCE (run cargo build --release first)" >&2
  exit 2
fi

install -Dm755 "$BIN_SOURCE" /usr/bin/zyvor-device-agent
ln -sfn /usr/bin/zyvor-device-agent /usr/bin/agentctl
install -Dm644 "$ROOT/packaging/systemd/zyvor-device-agent.service" /usr/lib/systemd/system/zyvor-device-agent.service
install -Dm644 "$ROOT/packaging/systemd/zyvor-device-agent.privsep.service.example" /usr/lib/systemd/system/zyvor-device-agent.privsep.service.example 2>/dev/null || true
install -d /etc/zyvor/device-agent/profiles /etc/zyvor/device-agent/plugins.d /etc/zyvor/device-agent/tls /etc/zyvor/device-agent/identity /var/lib/zyvor-device-agent /usr/lib/zyvor-device-agent/plugins
install -m644 "$ROOT"/profiles/*.toml /etc/zyvor/device-agent/profiles/
install -m755 "$ROOT/examples/i2c_temperature.py" /usr/lib/zyvor-device-agent/plugins/i2c_temperature.py
install -m644 "$ROOT/examples/plugins.d/i2c-temperature.json" /etc/zyvor/device-agent/plugins.d/i2c-temperature.json

if [[ ! -e /etc/zyvor/device-agent.toml ]]; then
  install -Dm640 "$ROOT/config/device-agent.example.toml" /etc/zyvor/device-agent.toml
fi

if [[ -d "$ROOT/web/dashboard/dist" ]]; then
  install -d /usr/share/zyvor-device-agent/dashboard
  cp -a "$ROOT/web/dashboard/dist/." /usr/share/zyvor-device-agent/dashboard/
fi

if [[ -n "$ENROLLMENT_TOKEN" ]]; then
  printf '%s\n' "$ENROLLMENT_TOKEN" >/etc/zyvor/device-agent/identity/enrollment-token
  chmod 600 /etc/zyvor/device-agent/identity/enrollment-token
  if [[ -n "$ENROLLMENT_URL" ]]; then
    if command -v python3 >/dev/null; then
      python3 - "$ENROLLMENT_URL" <<'PY'
import pathlib, re, sys
url = sys.argv[1]
path = pathlib.Path("/etc/zyvor/device-agent.toml")
text = path.read_text()
text = re.sub(r'(?m)^enabled = false$', "enabled = true", text, count=1)
text = re.sub(r'(?m)^server_url = ".*"$', f'server_url = "{url}"', text, count=1)
path.write_text(text)
PY
    fi
  fi
  zyvor-device-agent --config /etc/zyvor/device-agent.toml enroll || true
fi

if [[ "$SKIP_COMMISSION" -eq 0 ]]; then
  zyvor-device-agent --config /etc/zyvor/device-agent.toml commission || true
fi

echo "Installed Zyvor Device Agent. Review /etc/zyvor/device-agent.toml then: systemctl enable --now zyvor-device-agent"
