# Engineering Backlog

## v0.1.1 — Live hardware increment

- [x] background inventory refresh + cache
- [x] material hardware-change event stream (SSE)
- [x] bounded recent event history
- [x] actual IP-address discovery through `ip -j address`
- [x] RX/TX/error counters from sysfs
- [x] scheduled sensor sampling
- [x] canonical typed sensor sample envelope
- [x] per-plugin poll interval + Nodra publish flag
- [x] plugin absolute-path/executable/world-writable validation
- [x] plugin timeout + stdout size limit
- [x] real LM75/TMP102 I2C temperature reference plugin
- [x] retained Nodra inventory/status topics
- [x] per-sensor + event MQTT topics
- [x] Fleet address projection
- [x] expanded Prometheus metrics
- [x] live Sensors + event Diagnostics UX
- [x] install script scaffold

## P0 — Physical reference-hardware acceptance

- [ ] Select exact reference-hardware SKU and freeze expected device-tree nodes. (needs physical hardware)
- [ ] Install the reference plugin on the physical unit and confirm the real I2C bus/address. (needs physical hardware)
- [x] Add the first RS485/Modbus RTU adapter in `zyvorai/nodra` (not Device Agent).
- [x] Teach the existing `fleet-agent` to merge `/api/v1/integrations/fleet/inventory`.
- [ ] Hardware-in-loop test on the physical reference-hardware unit. (needs physical hardware)
- [ ] WAN-loss/reconnect demo with Nodra WAL replay. (needs physical hardware)

## P1 — Production hardening

- [x] Local bearer protection when binding beyond loopback (v0.1.4).
- [x] Enrollment token + mTLS device certs (v0.1.4 — `auth.mode = "mtls"`,
  `zyvor-device-agent enroll`/`identity`; client-side only, see
  `docs/MTLS_ENROLLMENT.md` for why Device Agent doesn't implement a CA).
- [x] TPM2/secure-element-backed key storage (v0.1.4 — `identity.backend =
  "tpm"`, `--features tpm2`, off by default; falls back to a software key at
  runtime if the TPM can't be opened. See `docs/TPM2_IDENTITY.md`).
- [x] Unix-domain-socket API mode (v0.1.4).
- [x] Linux capability/udev policy documentation per bus (v0.1.4 —
  `docs/HARDWARE_PERMISSIONS.md`).
- [x] Plugin executable owner allowlist and process resource limits (v0.1.4).
- [x] Signed release artifacts, SBOM, provenance and cosign workflow (v0.1.4).
- [x] Debian/RPM packages and clean uninstall path (v0.1.4 — amd64 only;
  `cargo-deb`/`cargo-generate-rpm`, both Cargo-metadata-driven; installing
  never auto-enables/starts the service; config is a conffile/noreplace and
  survives upgrade/removal). arm64 packages not yet built/validated.
- [x] ARM64 smoke test under QEMU in CI (v0.1.4 — and its own smoke-test
  command bug, present since it was added, only caught and fixed once
  `-D warnings` made the CI clippy gate meaningful).
- [x] Optional Linux udev/netlink event source to complement polling (v0.1.4
  — `--features hotplug`; a raw `NETLINK_KOBJECT_UEVENT` socket, not
  `udev`/`libudev.so`, so it adds no runtime library dependency).
- [x] Configurable thermal/counter health thresholds and alert events (v0.1.4).
- [x] Graceful config reload without a full restart (v0.1.4 — `SIGHUP`;
  `server.listen`/`unix_socket`/`dashboard_dir` and the Nodra/CAN-capture
  connections still need a restart, everything else applies live).
- [x] `seccomp` profiles for plugin subprocess execution (v0.1.4 —
  `plugins.seccomp_enabled`, a denylist of dangerous syscalls rather than a
  strict allowlist, since plugins have unknowable syscall needs).

## Explicitly outside Device Agent v0.x

Kubernetes dependency, industrial protocol meaning, Nodra WAL duplication, a
second Fleet agent, arbitrary remote shell, bootloader/partition OTA logic or
EdgeAI inference.
