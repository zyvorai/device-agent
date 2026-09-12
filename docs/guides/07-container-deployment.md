# 7. Container deployment

The bare-metal path in
[6. Deploying to production](06-deploying-to-production.md) builds on the
target device (or over SSH against it) with a full Rust/Node toolchain —
fine for a beefy gateway, heavy for a small ARM64 edge board. This guide
covers the alternative: pull a prebuilt, multi-arch (`linux/amd64` +
`linux/arm64`) image and run it, with no on-device build step at all.

Examples below use [Podman](https://podman.io/) — no persistent daemon to
run in the background, and it integrates directly with systemd (shown in
step 5), which suits a small always-on device better than running `dockerd`
as a second background service. The image is a standard OCI image, so
`docker run`/`docker pull` work identically if that's what you already have
installed.

## 1. Quick run

```bash
podman pull ghcr.io/zyvorai/device-agent:latest
podman run --rm -p 9188:9188 ghcr.io/zyvorai/device-agent:latest
```

No config mount needed for this first run — the image bakes
`config/device-agent.example.toml` in at `/etc/zyvor/device-agent.toml`
(`Config::load_or_default`'s default path), so it loads that rather than
truly running config-less; `ServerConfig::default()`'s `listen` and
`dashboard_dir` are kept identical to the example config's, so the
observable behavior matches the plain binary's built-in defaults either
way. Bind-mount your own file over that path (step 3) to actually run
config-less-equivalent or with different settings. Confirm it's up from
another terminal:

```bash
curl -sS http://127.0.0.1:9188/api/v1/health
curl -sS http://127.0.0.1:9188/api/v1/ready
```

Without any `--device`/`--network` flags, hardware discovery will mostly
come back empty — that's expected, and step 2 covers wiring in real buses.

## 2. Giving it access to real buses

The image runs as a non-root `zyvor` user by default (see the `Dockerfile`
comment) — fine for a container with no hardware access, but device-node
permissions on the host are typically root- or bus-group-owned, and
matching host bus-group GIDs into a container is fragile. The documented
path for real bus access is `--user root` plus explicit `--device` flags —
narrower and more auditable than `--privileged`:

| Bus | Host resource | Container flag |
|---|---|---|
| I2C | `/dev/i2c-N` | `--device=/dev/i2c-1` |
| GPIO | `/dev/gpiochipN` | `--device=/dev/gpiochip0` |
| SPI | `/dev/spidevN.N` | `--device=/dev/spidev0.0` |
| UART/serial | `/dev/ttyUSBN`, `ttyAMAN`, `ttySN` | `--device=/dev/ttyUSB0` |
| Watchdog | `/dev/watchdogN` | `--device=/dev/watchdog0` |
| CAN (health + read-only capture) | a netdevice + raw `AF_CAN` socket, not a `/dev` node | `--network host` |
| USB, thermal zones, network inventory | sysfs only (`/sys/bus/usb`, `/sys/class/thermal`, `/sys/class/net`) | none — sysfs is visible by default |

CAN needs `--network host` rather than a `--device` flag: SocketCAN
interfaces aren't device nodes, and moving a single netdevice into a
container's own network namespace (`ip link set can0 netns ...`) is
possible but far more fiddly to script reliably than just sharing the
host's network namespace outright. That does mean giving up network
namespace isolation for that container — worth knowing, not usually a
problem on a single-purpose edge device.

Example with I2C + GPIO:

```bash
podman run --rm -p 9188:9188 --user root \
  --device=/dev/i2c-1 --device=/dev/gpiochip0 \
  ghcr.io/zyvorai/device-agent:latest
```

## 3. Custom config

Bind-mount a config file over the default location instead of relying on
built-in defaults — same file format as
[2. Configuration & the API](02-configuration-and-api.md):

```bash
podman run --rm -p 9188:9188 --user root \
  -v /path/to/device-agent.toml:/etc/zyvor/device-agent.toml:ro,Z \
  ghcr.io/zyvorai/device-agent:latest
```

(the trailing `,Z` relabels the file for SELinux-enforcing hosts — such as
Fedora/RHEL-based images — and is harmless where SELinux isn't in play.)

## 4. Persistent state

`auth.mode = "bearer"`'s token hash, `auth.mode = "mtls"`'s enrolled
identity, and any custom plugin manifests all live under
`/etc/zyvor/device-agent/` inside the container — ordinary container
filesystem, gone the moment the container is removed. Give it a named
volume so re-running or updating the container doesn't force
re-enrollment:

```bash
podman volume create zyvor-device-agent-state
podman volume create zyvor-device-agent-var

podman run --rm -p 9188:9188 --user root \
  -v /path/to/device-agent.toml:/etc/zyvor/device-agent.toml:ro,Z \
  -v zyvor-device-agent-state:/etc/zyvor/device-agent:Z \
  -v zyvor-device-agent-var:/var/lib/zyvor-device-agent:Z \
  ghcr.io/zyvorai/device-agent:latest
```

Setting up bearer or mTLS auth itself doesn't change —
[3. Securing your agent](03-securing-your-agent.md) applies exactly as
written, including `enroll`/`identity` (run them with `podman exec` against
the running container, or `podman run --rm -it ... enroll` once against the
same volumes before starting `serve`). Only the file locations move onto
the volumes above instead of straight onto the host's `/etc/zyvor/`.

## 5. Going to production: systemd-supervised

For an always-on deployment you want boot-start, auto-restart, and
`journalctl` logs — the same guarantees the bare-metal path gets from
`packaging/systemd/zyvor-device-agent.service`. Use the container
equivalent, `packaging/container/zyvor-device-agent-container.service`:

```bash
sudo cp packaging/container/zyvor-device-agent-container.service \
  /etc/systemd/system/
# edit it: add the --device/--network flags your board needs (step 2),
# and point the config volume mount at your real config file (step 3)
sudo systemctl daemon-reload
sudo systemctl enable --now zyvor-device-agent-container
```

```bash
journalctl -u zyvor-device-agent-container -f    # logs
systemctl status zyvor-device-agent-container     # state
systemctl reload zyvor-device-agent-container     # SIGHUP config reload — same semantics as the bare-metal path
```

The shipped unit deliberately does **not** pass `--restart` to `podman run`
— systemd's own `Restart=always` already supervises the container, and the
two fight each other if both try to restart it.

Updating to a new image, with no rebuild and no SSH source sync:

```bash
podman pull ghcr.io/zyvorai/device-agent:latest
sudo systemctl restart zyvor-device-agent-container
```

If you'd rather not hand-maintain the unit file, `podman generate systemd
--new` (or Quadlet `.container` files, on newer Podman) can generate an
equivalent unit from a running container instead — the shipped file here is
just the reviewable, dependency-free default.

## 6. Container vs. bare-metal systemd — which one?

- **Container** wins when: you want the same image on amd64 and arm64
  today (the `.deb`/`.rpm` packages are amd64-only for now — see
  [`BACKLOG.md`](../BACKLOG.md)), or you'd rather not install a Rust/Node
  toolchain on the device at all.
- **Bare-metal systemd** wins when: a board needs many buses at once (no
  `--device` flag bookkeeping to maintain), or you need the optional
  `hotplug` feature's raw netlink socket in the host's own network
  namespace, which is simplest without a container boundary in the way.

Both talk to the same REST API and dashboard on port 9188, and everything
in [2](02-configuration-and-api.md) through
[5](05-writing-a-sensor-plugin.md) of this series applies unchanged either
way.
