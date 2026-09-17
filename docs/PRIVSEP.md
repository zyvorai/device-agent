---
hero:
  eyebrow: PRIVSEP
  title: 'Privilege separation: bus-access helper (design)'
---

Today the daemon opens GPIO/I2C/SPI/CAN (and optional camera) device nodes
**directly** and the stock systemd unit runs as `User=root`. Plugin
subprocesses can already drop via `plugins.run_as_uid` / `run_as_gid` (see
[`PLUGIN_PROTOCOL.md`](PLUGIN_PROTOCOL.md) and
[`HARDWARE_PERMISSIONS.md`](HARDWARE_PERMISSIONS.md)). What remains open is
privilege separation for the daemon's **own** bus access: an API process that
does not hold device-node credentials, talking to a small helper that does.

**Status:** design + config/feature scaffold. Default remains root + direct
bus access. No helper binary is spawned.

## Goals

1. Shrink the attack surface of the long-lived HTTP/MQTT process.
2. Keep discovery and capture behavior identical for operators who stay on
   the stock root unit.
3. Reuse the same group/udev story already documented for non-root daemon and
   plugins — the helper is the process that joins `i2c`/`gpio`/`spi`/`video`
   (and holds `CAP_NET_RAW` / `CAP_NET_ADMIN` for CAN), not the API daemon.

## Non-goals (this scaffold)

- Implementing the helper binary or IPC protocol
- Changing the default systemd unit away from `User=root`
- Replacing plugin `run_as_uid`/`run_as_gid` (orthogonal; keep both)

## Proposed shape

```text
┌─────────────────────────┐         Unix socket          ┌──────────────────────┐
│ zyvor-device-agent      │  ←── /run/.../bus.sock ──→   │ bus-helper (root or  │
│ (API, Nodra, Fleet)     │     length-prefixed RPC      │  bus-group + caps)   │
│ User=zyvor (future)     │                              │ opens /dev/i2c-*,    │
└─────────────────────────┘                              │ gpiochip*, can raw…  │
                                                         └──────────────────────┘
```

- **API daemon** — no ambient device access; talks only to the helper socket.
- **Helper** — short-lived or long-lived; allowlisted operations (read sysfs /
  open declared buses / CAN RX capture / camera open). No shell, no
  arbitrary `exec`.
- **Activation** — systemd socket activation or explicit spawn of
  `helper_path` at daemon start when `[privsep].enabled = true`.

## Config knobs (landed)

```toml
[privsep]
# Default false: daemon opens buses directly (today's root unit).
enabled = false
helper_path = "/usr/lib/zyvor-device-agent/bus-helper"
socket_path = "/run/zyvor-device-agent/bus.sock"
```

- Always parsed so TOML stays stable across builds.
- `--features privsep` marks the binary as "scaffold aware"; even with both
  the feature and `enabled = true`, **no helper is started yet** — startup
  logs a warning and bus access stays direct.
- Inspect via `zyvor_device_agent::privsep::status` (unit-tested).

## Migration path (future)

1. Ship helper binary + RPC for inventory-only reads.
2. Move CAN capture and camera open behind the helper.
3. Publish a non-root API unit that requires `privsep.enabled = true`.
4. Keep `packaging/systemd/zyvor-device-agent.nonroot.service.example` as the
   interim "one process, many groups" path until the helper exists.

## Relationship to non-root example unit

The opt-in non-root unit + udev rules in
`packaging/systemd/zyvor-device-agent.nonroot.service.example` are a **single
process** privilege drop. Privsep is the **two process** end state. Both are
documented; neither is default. See
[`HARDWARE_PERMISSIONS.md`](HARDWARE_PERMISSIONS.md).
