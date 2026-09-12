# Changelog

## 0.1.5-dev — 2026-09-12

- Add optional camera support (`--features camera`, off by default like
  `hotplug`/`tpm2`): live snapshot and live MJPEG video stream from an
  explicitly-allowlisted `/dev/video*`, following `can_capture.rs`'s
  opt-in/rate-limited/never-fatal posture — starts the device-discovery and
  live-viewing half of `docs/ROADMAP.md`'s "Edge AI bridge" line item
  (local inference/accelerator support stays a separate, still-unscoped
  future increment). New `[camera]` config (top-level, not nested under
  `[industrial]`), new `src/camera_capture.rs` (one dedicated thread per
  configured camera; prefers native MJPG passthrough, falls back to a
  pure-Rust YUYV→RGB→JPEG software encode via `jpeg-encoder` for cameras
  without it; bounded retry-with-backoff on error, unlike CAN capture's
  "fail once" policy, since USB replug is common). New endpoints
  `GET /api/v1/camera`, `GET /api/v1/camera/{id}/snapshot`,
  `GET /api/v1/camera/{id}/stream` (`multipart/x-mixed-replace`, renders in
  a plain `<img>` tag, no WebRTC/signaling) — the latter two get the same
  `?token=` query-auth carve-out the SSE routes already have, generalized
  to a path-prefix list (`src/auth/mod.rs`'s new
  `QUERY_TOKEN_PATH_PREFIXES`) since camera ids are dynamic. Nodra
  publishing (`camera.devices[].publish_to_nodra`) sends only
  `CameraCaptureStatus` health/presence, never frame bytes — a deliberate
  boundary documented with a code comment on `nodra::publisher_loop` so a
  future contributor doesn't "complete the parallel" with CAN's per-frame
  forwarding arm. `v4l` (default `v4l2` feature — raw kernel ioctls, no
  `libv4l.so`) and `jpeg-encoder` are both pure Rust; `v4l` is additionally
  gated to `target_os = "linux"` (V4L2 itself is Linux-only), with a no-op
  `spawn()` stub everywhere else mirroring `hardware::hotplug`'s pattern.
  The YUYV→RGB→JPEG pipeline has real unit-test coverage that runs
  cross-platform under `--features camera` (no Linux/hardware needed) by
  gating that one sub-module on `any(target_os = "linux", test)` rather
  than requiring Linux outright. `packaging/systemd/zyvor-device-agent.service`
  needed no `DeviceAllow=` change — confirmed `DevicePolicy=auto` with zero
  entries already permits root's `/dev/video*` access, documented with a
  comment so nobody "fixes" it unnecessarily. This dev machine is macOS, so
  the real V4L2 ioctl path (as opposed to the cross-platform encode-pipeline
  unit tests) is unverified here — it needs a Linux box with either a real
  camera or a `v4l2loopback` virtual device to test end to end.

- Add optional native TLS for the TCP listener: `server.tls.enabled` (off
  by default) serves plain HTTPS — no client certificate ever required,
  unlike `auth.mode = "mtls"` — so an ordinary browser can reach
  `https://<host>:9188/` directly. If no cert exists at
  `server.tls.cert_path`/`key_path`, one is generated automatically on
  first start (`src/tls.rs`, `rcgen::generate_simple_self_signed`, SANs:
  localhost/127.0.0.1/::1/hostname/detected local IP), mirroring
  `../fabric`'s `zyvor-fabricd::tls` module; a real cert can simply be
  mounted at the same paths instead, and an existing cert/key there is
  never overwritten. Independent of `auth.mode`: TLS and bearer/mtls/none
  auth compose (e.g. `server.tls.enabled = true` + `auth.mode = "bearer"`
  gives an encrypted transport with a required token, the recommended
  combination for a non-loopback bind). `auth.mode = "mtls"` still takes
  its own separate TLS path (`auth::mtls::load_server_config`) when set,
  since that one ties TLS to client-certificate verification.

## 0.1.4 — 2026-09-12

- Publish multi-arch (`linux/amd64` + `linux/arm64`) container images to
  `ghcr.io/zyvorai/device-agent` on every tagged release, keyless-signed
  with cosign and attested for build provenance, alongside the existing
  `.deb`/`.rpm`/tarball artifacts — new `container` job in
  `.github/workflows/release.yml`. `ci.yml`'s existing per-push build+smoke-test
  job (`--load`, local only) is unchanged; this is the first job that
  actually publishes an artifact. Adds a systemd-supervised alternative to
  the bare-metal deployment path (`packaging/container/zyvor-device-agent-container.service`,
  Podman-based) and a new tutorial,
  `docs/guides/07-container-deployment.md`, covering device/bus passthrough
  flags, config/state volumes, and running it under systemd — aimed at
  small ARM64 edge devices where an on-device Rust/Node build is
  impractical and the amd64-only `.deb`/`.rpm` packages don't apply yet.
- Add a numbered `docs/guides/` tutorial series (getting started;
  configuration and the API; securing the agent; industrial buses; writing
  a sensor plugin; deploying to production) and a README banner/badge
  header, replacing the plain-text architecture diagram with an SVG in the
  same visual family as the new banner. Docs only — no behavior change.
  Every command, endpoint, config key, and CLI flag referenced in the new
  guides was cross-checked against the current source
  (`src/api.rs`'s route table, `config/device-agent.example.toml`,
  `scripts/deploy-remote.sh`'s flag parsing) rather than written from
  memory.

- Add optional Linux hotplug event source: `--features hotplug` (off by
  default) opens a raw `NETLINK_KOBJECT_UEVENT` socket (`netlink-sys`, pure
  Rust) alongside the existing polling inventory refresh, and a kernel
  uevent for a tracked bus subsystem (`gpio`/`i2c`/`spidev`/`net`/`usb`/
  `tty`) triggers an immediate `hardware::collect_inventory` +
  `state.update_inventory` re-scan instead of waiting for the next poll
  tick - reusing the existing SSE hardware-change stream and threshold
  evaluation as-is. Deliberately not `udev`/`tokio-udev`: those need
  `libudev.so` at runtime, which the container image/`.deb`/`.rpm` don't
  otherwise depend on. Opening the socket is never fatal - a warning is
  logged once and the daemon falls back to polling-only. New
  `src/hardware/hotplug.rs`; `rust-native`'s CI job gains a second
  clippy/test/build pass with `--features hotplug` (no extra system
  library needed, unlike `tpm2`, so it didn't need its own job). Verified
  for real: sent a synthetic but correctly-formatted uevent over the real
  netlink multicast group and confirmed the daemon's inventory-refresh
  timestamp jumps immediately (well under the poll interval, which was
  set to 300s for the test) for a tracked subsystem (`i2c`) and does *not*
  move for an untracked one (`cpu`), proving both the trigger and the
  filter are real, not just compiled.
- Add optional TPM2-backed mTLS identity: `identity.backend = "tpm"`
  (`--features tpm2`, off by default, must never affect a plain `cargo
  build`) generates and signs the mTLS private key inside a TPM2 via
  `tss-esapi`'s `TransientKeyContext` instead of a PKCS#8 file on disk -
  what's persisted to `auth.mtls.key_file` is a small JSON envelope (the
  TPM's public key plus an encrypted private-key blob only that TPM can
  unwrap), and every signature (CSR at enroll time, every TLS handshake
  while serving) re-opens a TPM session rather than loading a private key
  into process memory. Falls back to a software key at runtime (with a
  warning) if the TPM can't be opened, since most dev/test boxes have none.
  Fixed to ECC P-256/ECDSA-SHA256, the combination every TPM2 chip and
  `swtpm` support well. New `src/identity` module: `DeviceIdentity` wraps
  either backend behind the same `rcgen::SigningKey` (CSR generation) and
  `rustls::sign::SigningKey` (TLS handshake signing) interfaces the
  existing enroll/mtls code already used, so neither needed a rewrite -
  only a backend behind them changed. CI gains a dedicated `rust-tpm2` job
  (installs `libtss2-dev`, builds/clippies/tests with `--features tpm2`);
  `rust-native`'s clippy step drops `--all-features` so it no longer
  silently depends on that job's system library. Live-verified against
  `swtpm` (the TPM2 emulator used where no physical TPM is available):
  ran `enroll` for real with `identity.backend = "tpm"`, confirmed the
  persisted key file is the JSON envelope (not a PKCS#8 key) and `identity`
  reports `backend: tpm`, then started `serve` in `auth.mode = "mtls"` and
  confirmed a real mTLS handshake succeeds - i.e. the TLS signature the
  emulated TPM produced verifies against the certificate's public key. See
  `docs/TPM2_IDENTITY.md`.
- Add `auth.mode = "mtls"` and `zyvor-device-agent enroll`/`identity`:
  client-side enrollment generates a keypair and CSR, submits them to
  `enrollment.server_url` with a single-use token, and persists the issued
  certificate/key. `serve` then terminates TLS itself (via `axum-server`'s
  rustls integration) using that identity and, when
  `auth.mtls.require_client_cert = true`, rejects any connection without a
  client certificate signed by `client_ca_file` at the handshake. Device
  Agent implements only this client side of the protocol - Fleet's
  enrollment-token system today issues an opaque bearer token, not a signed
  certificate, and a production CA (key custody, revocation, rotation) is a
  separate, security-sensitive project of its own; see
  `docs/MTLS_ENROLLMENT.md`. New dependencies: `rcgen` (CSR generation),
  `reqwest` (rustls-backed, for submitting the CSR), `axum-server`+`rustls`
  (serving mTLS), `rustls-pemfile` and `x509-parser` (reading the resulting
  identity back for `identity`). Automatic rotation and CRL/OCSP revocation
  are explicit non-goals for this milestone - reissue manually with
  `enroll --force`. TPM2/secure-element-backed key storage is a separate,
  optional follow-up.
- Add signed `.deb`/`.rpm` packages (amd64) to the release pipeline, built
  via `cargo-deb`/`cargo-generate-rpm` from new `[package.metadata.deb]`/
  `[package.metadata.generate-rpm]` tables in `Cargo.toml` (both fully
  Cargo-metadata-driven, no separate packaging manifest). Asset layout
  mirrors `scripts/install.sh`. Installing the package never enables or
  starts the systemd unit — `cargo-deb`'s `systemd-units` integration is
  configured with `enable = false`/`start = false`, and the generated
  `.deb` carries no maintainer scripts at all as a result; the rpm's only
  scriptlet is `systemctl daemon-reload`. `/etc/zyvor/device-agent.toml`
  is a conffile (dpkg) / `%config(noreplace)` (rpm), so a locally-modified
  config survives both an upgrade and a straight reinstall, and
  `dpkg -r`/`rpm -e` leave it and the profiles/plugin manifests on disk —
  all verified for real: built both packages on the reference Linux host,
  `dpkg -i`'d the `.deb` on top of an already-running manually-deployed
  instance (confirmed `--force-confold`'s "Keeping old config file as
  default" preserves an operator edit), then `dpkg -r`'d it and confirmed
  the config/profiles remained; the `.rpm` was verified via an isolated
  `rpm --root` install (file layout, permissions, no enable/start
  scriptlet) since this host runs a Debian-family package manager, not
  rpm, day to day. arm64 packages aren't built yet — cross-packaging
  wasn't validated in this pass, so it's tracked as a follow-up rather
  than blocking amd64 on it.
- Add config hot-reload: `SIGHUP` re-reads the config file and applies
  `auth.*`, `thresholds.*`, `plugins.*`, `fleet.*` and the parts of
  `industrial.*`/`nodra.*` read fresh per-request/tick, without a restart.
  `server.listen`/`unix_socket`/`dashboard_dir` (bound once at startup) and
  the Nodra MQTT connection/CAN-capture socket set (opened once at their own
  startup) still need a restart — `SIGHUP` logs a clear warning when one of
  those changed. A config file that fails to parse is logged and ignored,
  keeping the daemon on its last-known-good config. `AppState.config` moves
  from a plain field to an `arc_swap::ArcSwap<Config>` to make this possible.
- Add opt-in `plugins.seccomp_enabled` (Linux only): installs a seccomp-bpf
  denylist in the plugin subprocess (`ptrace`, `mount`/`umount2`/
  `pivot_root`, `reboot`/`kexec_load`, module loading/unloading, `acct`,
  `swapon`/`swapoff`, `bpf`, `perf_event_open`, `keyctl`/`add_key`/
  `request_key`, `setns`, `unshare` → `EPERM`, everything else allowed) on
  top of the existing identity drop and rlimits, as defense-in-depth against
  a compromised or malicious plugin binary.

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
- CI: `rust-native` now runs `cargo fmt --all -- --check` and
  `cargo clippy --all-features -- -D warnings` (previously `cargo clippy
  --all-targets` alone, which didn't fail the build on warnings).
- Add ESLint (flat config, typescript-eslint + react-hooks) to the dashboard,
  wired into CI; also adds Prettier as an available `npm run format` (not yet
  enforced in CI — the existing code predates it and a full reformat is a
  separate decision).
- Add `docker` to `.github/dependabot.yml` so the `Dockerfile`'s base images
  get automated update PRs too.
- Release notes now surface the matching `CHANGELOG.md` section as the GitHub
  Release body, instead of only the file list.
- Add opt-in configurable health thresholds (`[thresholds]`): thermal zones
  and SocketCAN controller error counters are checked once per inventory
  refresh, emitting `threshold.breached`/`threshold.recovered` on the SSE
  event stream (and the Nodra agent-event topic, if enabled) only on the
  edge transition.
- Fix: `src/plugins.rs` had an unused `std::os::unix::process::CommandExt`
  import on Linux (`tokio::process::Command::pre_exec` doesn't need it) that
  CI's `cargo clippy` never actually caught until `-D warnings` was added in
  this release — and that only Linux CI can catch at all, since the import
  is `#[cfg(target_os = "linux")]`-gated and this session's local clippy
  verification runs on macOS.
- Split `src/main.rs` into a `src/lib.rs` library crate (re-exporting `api`,
  `auth`, `config`, `hardware`, `integrations`, `model`, `plugins`, `profile`,
  `state`) plus a thin binary, so `tests/` can build the real `Router` and
  drive it end-to-end via `tower::ServiceExt::oneshot` - previously every
  module was private to the binary target and unreachable from outside it.
- Add unit tests for `plugins::validate()` (owner/directory allowlists,
  world-writable/non-executable/non-absolute rejection) and the plugin
  hardening config-gating logic, `auth::check_bearer` (missing/wrong/correct
  token, non-Bearer scheme, unconfigured-hash fail-closed), and
  `auth::uds::peer_is_allowed` (uid/gid allow-list matching, the empty-list
  deny-everyone default) - previously all at zero coverage despite being the
  highest-risk code (external process execution, network auth).
- Add `tests/api_auth.rs`: the first true HTTP-level integration test,
  proving the auth middleware wiring works end-to-end against the real
  `Router` (health/dashboard-shell reachable without a token, inventory
  correctly gated in bearer mode) rather than only unit-testing each piece.
- Add `docs/QUICKSTART.md` (packaged/systemd install, bearer auth, dashboard
  token connection, Prometheus scrape config, end to end) and
  `docs/observability/` (a Prometheus scrape-config example and a starter
  Grafana dashboard built from the real `/metrics` metric names).
- `docs/ROADMAP.md`: move `v0.4` (OTA executor) and `v0.5` (Edge AI bridge)
  into a "Later / not yet scoped" section, resolving a contradiction with
  `docs/BACKLOG.md`'s "explicitly outside Device Agent v0.x" list (which
  disclaimed both) — neither doc change implements anything, just removes
  the disagreement between them. Mark `v0.2`'s Unix-socket/RBAC, plugin
  privilege separation and health-threshold lines done.
- `docs/BACKLOG.md`: check off everything shipped this release; split the
  bearer/mTLS P1 line now that bearer is done and mTLS is separately
  scoped; add previously-untracked `seccomp` profiles (mentioned only in
  `docs/ARCHITECTURE.md` until now) and config hot-reload as open items.

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
- Add industrial acceptance guide and Nodra Modbus RTU adapter contract.

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
