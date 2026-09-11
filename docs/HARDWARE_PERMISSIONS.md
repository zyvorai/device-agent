# Hardware bus permissions

Device Agent discovers hardware buses read-only over sysfs/procfs and `/dev` device
nodes (see `docs/ARCHITECTURE.md` and `docs/INDUSTRIAL_BUSES.md` for CAN specifically).
Today's `packaging/systemd/zyvor-device-agent.service` runs the daemon itself as
`User=root`, which sidesteps every group-membership question below for the daemon's
*own* discovery path — root can read any device node regardless of group. This doc
exists for two other cases:

1. **Sensor plugins** (`plugins.rs`) can be dropped to a non-root uid/gid via
   `plugins.run_as_uid`/`run_as_gid` (see `docs/PLUGIN_PROTOCOL.md`). That dropped-to
   account needs its own access to whichever device node the plugin actually opens.
2. **Any future move of the daemon itself off `User=root`** — not done today (no code
   path currently needs privileged *write* access to a bus), but if it happens, every
   bus the daemon polls needs to be re-derived from this table into
   `SupplementaryGroups=` in the unit file.

## Bus reference

| Bus | Device node glob | Typical group | Example udev rule | Notes |
|---|---|---|---|---|
| GPIO | `/dev/gpiochip*` | `gpio` (not present on stock Debian — must be created) | `SUBSYSTEM=="gpio", GROUP="gpio", MODE="0660"` | Discovery is sysfs-based (`/sys/class/gpio`) and read-only; no bus access needed for inventory alone. |
| I2C | `/dev/i2c-*` | `i2c` (create if absent) | `KERNEL=="i2c-[0-9]*", GROUP="i2c", MODE="0660"` | The reference `examples/i2c_temperature.py` plugin needs read/write on the specific `/dev/i2c-N` its manifest names. |
| SPI | `/dev/spidev*` | `spi` (create if absent) | `SUBSYSTEM=="spidev", GROUP="spi", MODE="0660"` | Discovered (`src/hardware/buses.rs`) but no shipped plugin uses it yet. |
| CAN | netdevice (`can0`, `vcan0`, …), plus `/dev/can*` on some drivers | n/a — netdevices, not device-node permissions | — | Governed by `CAP_NET_ADMIN`/`CAP_NET_RAW`, already in `AmbientCapabilities`/`CapabilityBoundingSet` in the systemd unit. See `docs/INDUSTRIAL_BUSES.md` and `docs/CAN_CAPTURE.md`. |
| Watchdog | `/dev/watchdog*` | none by default (root-only, `0600`) | — | Read-only presence check today; no plugin or capability currently touches it. |

## Setting up a dropped-privilege plugin account

```bash
useradd --system --no-create-home --shell /usr/sbin/nologin zyvor-plugin
usermod -aG i2c zyvor-plugin   # add whichever bus groups the plugin actually needs
id zyvor-plugin                 # note the uid/gid
```

Then in `device-agent.toml`:

```toml
[plugins]
run_as_uid = 999   # the uid from `id zyvor-plugin`
run_as_gid = 999
```

The daemon clears the dropped process's supplementary groups before exec (see
`plugins.rs::apply_privilege_drop`), so group membership must come from the target
account's **primary** gid or be granted explicitly — a plugin that needs both `i2c`
and `dialout` access, for example, needs a dedicated group that has both, not reliance
on inherited supplementary groups.

## Hardening checklist

- Today: daemon runs as root, so this table matters only for plugin subprocesses.
- If the daemon ever moves off `User=root`: every bus row above needs its group added
  to `SupplementaryGroups=` in `packaging/systemd/zyvor-device-agent.service`, and the
  CAN row's capabilities need to move from `AmbientCapabilities` (works for any uid)
  to being re-verified against the new non-root user.
- `NoNewPrivileges=true` is already set — a plugin dropped via `run_as_uid`/`run_as_gid`
  cannot regain privileges even if its binary is later replaced with something
  setuid-root, which is the property that makes the privilege drop meaningful defense
  in depth rather than cosmetic.
