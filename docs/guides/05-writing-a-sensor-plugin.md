# 5. Writing a sensor plugin

A plugin is just an executable plus a JSON manifest — Device Agent runs the
command directly (never through a shell), waits for one JSON object on
stdout, and normalizes it into a canonical sample. This guide builds the
simplest possible plugin end to end, then points at the real I²C reference
plugin for the next step up. Full contract:
[`PLUGIN_PROTOCOL.md`](../PLUGIN_PROTOCOL.md).

## 1. A minimal plugin

`examples/temperature-demo.sh` is already exactly this — a shell one-liner
that prints a simulated reading:

```sh
#!/usr/bin/env sh
set -eu
printf '{"sensor":"demo-temperature","value":%s,"unit":"celsius","quality":"simulated"}\n' "${ZYVOR_DEMO_TEMP:-24.8}"
```

Run it by hand first — this is exactly what Device Agent will execute:

```bash
chmod +x examples/temperature-demo.sh
./examples/temperature-demo.sh
# {"sensor":"demo-temperature","value":24.8,"unit":"celsius","quality":"simulated"}
```

A plugin can also return a `readings` array in one response when a single
transaction produces multiple measurements (e.g. temperature + humidity
from one I²C combo sensor) — see `PLUGIN_PROTOCOL.md` for that shape.

## 2. Write the manifest

Manifests live in the directory named by `[plugins] directory` in your
config (`plugins.d` in the example config). Create one:

```json
{
  "name": "temperature-demo",
  "version": "0.1.0",
  "command": "/absolute/path/to/examples/temperature-demo.sh",
  "args": [],
  "capabilities": ["temperature"],
  "sensor_id": "demo-temperature",
  "enabled": true,
  "poll_interval_seconds": 5,
  "publish_to_nodra": false
}
```

By default, Device Agent requires an **absolute** command path, rejects
commands that aren't executable regular files, rejects world-writable
executables, and caps captured stdout — a plugin manifest pointing at a
relative path or a script your build didn't `chmod +x` will fail
validation rather than silently doing nothing.

## 3. See it picked up

```bash
cargo run -- --config /tmp/device-agent.toml plugins
```

lists every manifest and its validation state. If it's valid and enabled:

```bash
cargo run -- --config /tmp/device-agent.toml sample temperature-demo
```

runs it once immediately from the CLI, no running server needed. With
`serve` running, the same thing is available over HTTP and feeds the
dashboard's **Sensors** page:

```bash
curl -sS -X POST http://127.0.0.1:9188/api/v1/plugins/temperature-demo/sample
curl -sS http://127.0.0.1:9188/api/v1/sensors/demo-temperature | jq .
```

The daemon polls it automatically every `poll_interval_seconds` from then
on, and — if `publish_to_nodra = true` — publishes each sample to Nodra.

## 4. Go from simulated to real hardware

`examples/i2c_temperature.py` is the shipped reference for this next step:
a real LM75/TMP102 I²C read against an explicit bus and address, with
`--self-test` for decoding logic verification without touching hardware
(see [1. Getting started](01-getting-started.md), step 5). Its manifest,
`examples/plugins.d/i2c-temperature.json`, is a good template — copy it and
change `--bus`/`--address`/`--sensor-id` for your board. Notice it does
**not** scan the I2C bus: some I2C devices react badly to unsolicited
probing, so the plugin only ever touches the bus/address explicitly given
in its manifest. Follow that pattern for any new hardware-backed plugin.

## 5. Harden it for production (optional)

All opt-in, all off by default:

```toml
[plugins]
run_as_uid = 1500
run_as_gid = 1500
allowed_owners = ["1500"]
allowed_directories = ["/usr/lib/zyvor-device-agent/plugins"]
max_memory_bytes = 67108864
max_cpu_seconds = 5
max_processes = 4
seccomp_enabled = true
```

`run_as_uid`/`run_as_gid` drop the plugin subprocess to a lower-privilege
identity (the daemon itself still runs as root by default) — see
[`HARDWARE_PERMISSIONS.md`](../HARDWARE_PERMISSIONS.md) for which device
group (e.g. `i2c`, `dialout`) that identity needs to actually reach the
bus. `allowed_directories` is resolved through symlinks, so it can't be
bypassed with `../` traversal. `seccomp_enabled` installs a fixed syscall
denylist (`ptrace`, `mount`, `reboot`, kernel-module loading, etc.) as
defense-in-depth on top of the identity drop and rlimits — not a
replacement for them. Full rationale in `PLUGIN_PROTOCOL.md`'s "Optional
hardening" section.

## Next steps

- [`PLUGIN_PROTOCOL.md`](../PLUGIN_PROTOCOL.md) — the complete manifest
  schema and safety rules.
- [`HARDWARE_PERMISSIONS.md`](../HARDWARE_PERMISSIONS.md) — device-group
  membership for dropped-privilege plugins.
- [6. Deploying to production](06-deploying-to-production.md) — shipping
  plugin manifests as part of a package/deployment.
