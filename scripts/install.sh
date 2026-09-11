#!/usr/bin/env bash
set -euo pipefail

ROOT="${DESTDIR:-}"
BIN_SOURCE="${BIN_SOURCE:-target/release/zyvor-device-agent}"

if [[ ! -x "$BIN_SOURCE" ]]; then
  echo "missing executable: $BIN_SOURCE (run cargo build --release first)" >&2
  exit 2
fi

install -Dm755 "$BIN_SOURCE" "$ROOT/usr/bin/zyvor-device-agent"
install -Dm644 packaging/systemd/zyvor-device-agent.service "$ROOT/usr/lib/systemd/system/zyvor-device-agent.service"
install -d "$ROOT/etc/zyvor/device-agent/profiles" "$ROOT/etc/zyvor/device-agent/plugins.d" "$ROOT/usr/lib/zyvor-device-agent/plugins"
install -m644 profiles/*.toml "$ROOT/etc/zyvor/device-agent/profiles/"
install -m755 examples/i2c_temperature.py "$ROOT/usr/lib/zyvor-device-agent/plugins/i2c_temperature.py"
install -m644 examples/plugins.d/i2c-temperature.json "$ROOT/etc/zyvor/device-agent/plugins.d/i2c-temperature.json"

if [[ ! -e "$ROOT/etc/zyvor/device-agent.toml" ]]; then
  install -Dm640 config/device-agent.example.toml "$ROOT/etc/zyvor/device-agent.toml"
fi

if [[ -d web/dashboard/dist ]]; then
  install -d "$ROOT/usr/share/zyvor-device-agent/dashboard"
  cp -a web/dashboard/dist/. "$ROOT/usr/share/zyvor-device-agent/dashboard/"
fi

echo "Installed Zyvor Device Agent. The I2C reference plugin is installed disabled; edit its manifest after confirming the physical bus/address."
