---
hero:
  eyebrow: HARDWARE PERMISSIONS
  title: Hardware bus permissions
---

Device Agent discovers hardware buses read-only over sysfs/procfs and `/dev` device
nodes (see `docs/ARCHITECTURE.md` and `docs/INDUSTRIAL_BUSES.md` for CAN specifically).
Today's `packaging/systemd/zyvor-device-agent.service` runs the daemon itself as
`User=root`, which sidesteps every group-membership question below for the daemon's
*own* discovery path — root can read any device node regardless of group. This doc
exists for two other cases:

1. **Sensor plugins** (`plugins.rs`) can be dropped to a non-root uid/gid via
   `plugins.run_as_uid`/`run_as_gid` (see `docs/PLUGIN_PROTOCOL.md`). That dropped-to
   account needs its own access to whichever device node the plugin actually opens.
2. **Optional non-root daemon** — default unit stays `User=root`. An opt-in example
   lives at `packaging/systemd/zyvor-device-agent.nonroot.service.example` with matching
   `packaging/udev/99-zyvor-device-agent.rules`. Every bus the daemon polls must be
   covered by those groups (see table) before switching.

## Bus reference

| Bus | Device node glob | Typical group | Example udev rule | Notes |
|---|---|---|---|---|
| GPIO | `/dev/gpiochip*` | `gpio` (not present on stock Debian — must be created) | `SUBSYSTEM=="gpio", GROUP="gpio", MODE="0660"` | Discovery is sysfs-based (`/sys/class/gpio`) and read-only; no bus access needed for inventory alone. |
| I2C | `/dev/i2c-*` | `i2c` (create if absent) | `KERNEL=="i2c-[0-9]*", GROUP="i2c", MODE="0660"` | The reference `examples/i2c_temperature.py` plugin needs read/write on the specific `/dev/i2c-N` its manifest names. |
| SPI | `/dev/spidev*` | `spi` (create if absent) | `SUBSYSTEM=="spidev", GROUP="spi", MODE="0660"` | Discovered (`src/hardware/buses.rs`) but no shipped plugin uses it yet. |
| CAN | netdevice (`can0`, `vcan0`, …), plus `/dev/can*` on some drivers | n/a — netdevices, not device-node permissions | — | Governed by `CAP_NET_ADMIN`/`CAP_NET_RAW`, already in `AmbientCapabilities`/`CapabilityBoundingSet` in the systemd unit. See `docs/INDUSTRIAL_BUSES.md` and `docs/CAN_CAPTURE.md`. |
| Watchdog | `/dev/watchdog*` | none by default (root-only, `0600`) | — | Read-only presence check today; no plugin or capability currently touches it. |
| Camera (`--features camera`) | `/dev/video*` | `video` (create if absent) | `SUBSYSTEM=="video4linux", GROUP="video", MODE="0660"` | Opened directly by the daemon (`src/camera_capture.rs`), not a plugin subprocess — root sidesteps this like every other row above. `packaging/systemd/zyvor-device-agent.service`'s `DevicePolicy=auto` has no `DeviceAllow=` entries, so no cgroup device filter blocks this today; the `video` group only matters for a future de-privileged daemon. |

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

## Non-root daemon (opt-in)

```bash
# 1) groups + udev
sudo groupadd --system gpio; sudo groupadd --system i2c; sudo groupadd --system spi
sudo cp packaging/udev/99-zyvor-device-agent.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger

# 2) service user
sudo useradd --system --no-create-home --shell /usr/sbin/nologin zyvor
sudo usermod -aG gpio,i2c,spi,video,dialout zyvor
sudo chown -R zyvor:zyvor /var/lib/zyvor-device-agent /run/zyvor-device-agent

# 3) swap unit (example — review SupplementaryGroups for your board)
sudo cp packaging/systemd/zyvor-device-agent.nonroot.service.example \
  /etc/systemd/system/zyvor-device-agent.service
sudo systemctl daemon-reload && sudo systemctl restart zyvor-device-agent
```

Validate with `id zyvor` and opening the same `/dev/*` nodes the agent needs.
Keep the stock root unit for boards that still require privileged setup paths.

## Hardening checklist

- Default: daemon runs as root, so this table matters primarily for plugin subprocesses.
- Non-root example: use the packaging paths above; keep `CAP_NET_ADMIN`/`CAP_NET_RAW`
  for CAN; re-verify every bus row against the `zyvor` user on the target board.
- `NoNewPrivileges=true` is already set — a plugin dropped via `run_as_uid`/`run_as_gid`
  cannot regain privileges even if its binary is later replaced with something
  setuid-root, which is the property that makes the privilege drop meaningful defense
  in depth rather than cosmetic.
- `plugins.seccomp_enabled = true` adds a further layer: even a plugin binary that's
  compromised or malicious can't `ptrace` another process, load a kernel module, or
  remount/pivot the filesystem — see `docs/PLUGIN_PROTOCOL.md` for the exact denylist.

## Daemon bus privilege separation (open)

Plugin drops and the optional non-root unit above are not a two-process
helper. The design for a dedicated bus-access helper (API daemon
unprivileged, helper holds device groups/caps) lives in
[`docs/PRIVSEP.md`](PRIVSEP.md). Config knobs (`[privsep]`) and
`--features privsep` are scaffold only: default remains `User=root` with
direct bus opens, and no helper binary is spawned yet.
