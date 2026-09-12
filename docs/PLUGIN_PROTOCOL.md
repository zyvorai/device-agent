# Sensor Plugin Protocol v1

A plugin is an executable plus a JSON manifest. Device Agent launches the
command directly (never through a shell) with `ZYVOR_PLUGIN_PROTOCOL=v1` and
expects one JSON object on stdout.

## Single-reading form

```json
{
  "sensor": "cabinet-temperature",
  "value": 31.2,
  "kind": "temperature",
  "unit": "celsius",
  "quality": "good",
  "labels": { "bus": "/dev/i2c-1", "address": "0x48" }
}
```

The daemon wraps that response in a canonical envelope containing the plugin
name, sensor ID, collection time, success state, normalized readings and raw
plugin output. The canonical envelope is used by REST, the dashboard and Nodra.

A plugin may instead return a `readings` array when one transaction produces
multiple measurements:

```json
{
  "sensor": "environment-1",
  "quality": "good",
  "readings": [
    { "name": "temperature", "kind": "temperature", "value": 31.2, "unit": "celsius" },
    { "name": "humidity", "kind": "humidity", "value": 62.1, "unit": "percent" }
  ]
}
```

## Manifest

```json
{
  "name": "i2c-temperature",
  "version": "0.1.1",
  "command": "/usr/lib/zyvor-device-agent/plugins/i2c_temperature.py",
  "args": ["--bus", "/dev/i2c-1", "--address", "0x48"],
  "capabilities": ["temperature", "i2c"],
  "sensor_id": "cabinet-temperature",
  "enabled": true,
  "poll_interval_seconds": 5,
  "publish_to_nodra": true
}
```

## Safety rules

By default Device Agent requires an absolute plugin command, rejects commands
that are not executable regular files, rejects world-writable executables and
caps captured stdout. A hung plugin is killed after the configured timeout.

Device Agent does **not** scan I2C addresses automatically. Some I2C devices
react badly to probing. The included LM75/TMP102 reference plugin only touches
the bus and address explicitly configured in its manifest.

The process boundary is deliberate. High-rate protocols belong in Nodra
adapters; Device Agent plugins are for bounded hardware/sensor sampling.

## Optional hardening (v0.1.4+, all opt-in)

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

- `run_as_uid`/`run_as_gid` (Linux only): drop the plugin subprocess to this
  identity before exec instead of inheriting the daemon's own (today: root).
  See `docs/HARDWARE_PERMISSIONS.md` for the device-group implications.
- `allowed_owners`: reject plugin commands not owned by one of these uids
  (numeric) or usernames.
- `allowed_directories`: reject plugin commands whose canonicalized path
  isn't under one of these prefixes — resolved through symlinks, so this
  can't be bypassed with a `../` traversal or a symlink pointing elsewhere.
- `max_memory_bytes`/`max_cpu_seconds`/`max_processes` (Linux only):
  `RLIMIT_AS`/`RLIMIT_CPU`/`RLIMIT_NPROC` applied to the subprocess — the
  last one in particular guards against a plugin that fork-bombs.
- `seccomp_enabled` (Linux only): installs a seccomp-bpf filter in the
  subprocess that returns `EPERM` for a fixed denylist of dangerous syscalls
  (`ptrace`, `mount`/`umount2`/`pivot_root`, `reboot`/`kexec_load`,
  `init_module`/`finit_module`/`delete_module`, `acct`, `swapon`/`swapoff`,
  `bpf`, `perf_event_open`, `keyctl`/`add_key`/`request_key`, `setns`,
  `unshare`) and allows everything else. Deliberately a denylist, not an
  allowlist — plugins are arbitrary external scripts/binaries with
  unknowable syscall needs, so a strict allowlist isn't safe to ship as a
  default. Layered on top of the identity drop and rlimits above as
  defense-in-depth, not a replacement for them.

All of these default to unrestricted/unlimited/disabled, matching pre-v0.1.4
behavior.
