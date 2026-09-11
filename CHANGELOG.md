# Changelog

## 0.1.4-dev — 2026-09-12

- Add bearer-token API auth (`auth.mode = "bearer"`); every route except
  `/api/v1/health` is unauthenticated by default (`mode = "none"`) — same as before.
- Add an additional Unix-domain-socket API listener with kernel peer-credential
  (uid/gid) RBAC, alongside the existing TCP listener.
- Add opt-in plugin privilege drop (`plugins.run_as_uid`/`run_as_gid`) so sensor
  plugin subprocesses no longer have to inherit the daemon's root identity.
- `scripts/deploy-remote.sh --auth-mode bearer` generates and installs a bearer
  token the same way `../fabric` handles its admin password; refuses to bind a
  non-loopback address with no auth configured.
- Add `docs/HARDWARE_PERMISSIONS.md` covering GPIO/I2C/SPI/CAN device-node
  permissions for dropped-privilege plugins.
- CI: the `container` job now actually boots the built amd64 and arm64 (via QEMU)
  images and checks `--version`/`doctor` run correctly, instead of only
  cross-building them.
- Release pipeline: generate CycloneDX SBOMs (Rust + dashboard), sign checksums
  and SBOMs with keyless `cosign`, and attach a SLSA build-provenance attestation.
- Add opt-in plugin hardening: `allowed_owners`/`allowed_directories` command
  allowlists, and `max_memory_bytes`/`max_cpu_seconds`/`max_processes` rlimits.
- Fix: the bundled dashboard now supports bearer auth (token entry UI, sends
  `Authorization: Bearer` on every request, and a scoped `?token=` fallback for
  the two SSE streams) — previously turning on `auth.mode = "bearer"` silently
  broke the dashboard with no way to authenticate.
- Fix: the dashboard's own static shell (`index.html`, JS, CSS) is now exempt
  from bearer auth — it was previously gated along with the API, which meant
  the browser couldn't even load the app far enough to show the token prompt
  above. Only `/api/*` and `/metrics` require auth; the shell carries no data.
- Add graceful shutdown: SIGINT/SIGTERM now drain in-flight HTTP/SSE
  connections on both the TCP and Unix-socket listeners before exiting, and
  stop the inventory-refresh, plugin-scheduler, CAN-capture and Nodra-publisher
  background loops cleanly instead of just dropping them. systemd unit gains
  `TimeoutStopSec=15` to give the drain time to finish before SIGKILL.
- Add `GET /api/v1/ready`: readiness (is the background inventory refresh loop
  still ticking?), distinct from `/api/v1/health` (pure liveness) and
  `/api/v1/doctor` (deep hardware diagnostics). Exempt from auth by default,
  alongside `/api/v1/health`, so probes never need a token.
- `scripts/deploy-remote.sh --auth-mode bearer` now also prints a ready-to-paste
  Prometheus scrape-config snippet for `/metrics`, which requires the same
  bearer token.
- Add opt-in CORS (`server.cors`, off by default) and rate limiting
  (`server.rate_limit`, on by default with generous per-IP limits) to the TCP
  listener. Neither applies to the Unix-socket listener.
- Container image now runs as a non-root `zyvor` user by default and adds a
  `HEALTHCHECK` against `/api/v1/ready`; real GPIO/I2C/CAN bus access still
  goes through the systemd deployment, which intentionally stays root.
- Fix: the `container` CI job's smoke test passed the entrypoint binary's own
  path as a CLI argument to itself (`docker run <image> /usr/bin/zyvor-device-agent
  --version` when the image's `ENTRYPOINT` is already that binary), so it had
  been failing since it was added in this release and was never actually green.

## 0.1.3-dev — 2026-09-11

- Add disabled-by-default, RX-only SocketCAN frame capture with explicit interface allowlists.
- Preserve classic CAN, 29-bit extended IDs and CAN-FD BRS/ESI metadata without protocol decoding.
- Add bounded frame history, accepted-frame rate limiting and capture health/error counters.
- Add `/api/v1/can/capture`, `/can/frames/recent` and SSE `/can/frames/stream`.
- Publish raw frames to per-interface Nodra topics when explicitly enabled.
- Add live CAN frame view to the Apple-style Industrial cockpit.
- Add Nodra J1939 hand-off contract; PGN/source/destination semantics stay out of Device Agent.

## 0.1.2-dev — 2026-09-11

- Add industrial bus inventory without turning Device Agent into a protocol gateway.
- Add read-only SocketCAN controller state, bitrate/CAN-FD bitrate and error telemetry.
- Detect physical vs virtual CAN and surface BUS-OFF in `doctor`.
- Add passive serial/RS485 awareness from explicit config and Linux device-tree properties.
- Add `/api/v1/industrial`, `/industrial/can` and `/industrial/serial`.
- Add CAN/RS485 Prometheus metrics and Fleet metadata.
- Publish retained industrial status into Nodra MQTT topics.
- Add Apple-style Industrial cockpit page.
- Add Minewing industrial acceptance guide and Nodra Modbus RTU adapter contract.

## 0.1.1-dev — 2026-09-11

- Continuous cached hardware inventory refresh independent of browser/API traffic.
- Material hardware-change events with SSE and bounded recent event history.
- Scheduled external sensor sampling with canonical typed `SensorSample` envelopes.
- Plugin executable validation, timeout enforcement and stdout size limits.
- Real LM75/TMP102 Linux I2C temperature reference plugin with no bus scanning.
- Nodra retained inventory/status, per-sensor topics and agent event topics while preserving the v0.1 telemetry topic.
- Network IP address and RX/TX/error counter collection.
- Fleet projection now includes discovered addresses.
- Expanded Prometheus metrics for inventory generations, buses and sensor health.
- Live Sensors and Diagnostics/Event Stream surfaces in the local Zyvor dashboard.
- Package installer scaffold for systemd, profiles and the disabled-by-default I2C reference plugin.

## 0.1.0-dev — 2026-09-11

- Initial Apache-2.0 repository scaffold.
- ARM64-first Linux hardware inventory and board profiles.
- GPIO/I2C/SPI/UART/CAN/USB/watchdog discovery.
- REST API and Prometheus metrics.
- Sensor plugin process contract.
- Nodra MQTT telemetry publishing.
- Fleet-compatible local inventory projection without duplicating `fleet-agent`.
- Apple-inspired Zyvor local hardware cockpit.
- systemd, OCI, CI, multi-arch build and release scaffolding.
