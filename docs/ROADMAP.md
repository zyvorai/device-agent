---
hero:
  eyebrow: ROADMAP
  title: Roadmap
---

## v0.1 — Hardware Edge MVP

- Linux hardware identity/inventory, physical buses and thermal discovery
- REST, Prometheus, external sensor-plugin process contract
- Nodra MQTT publishing and Fleet inventory projection
- systemd/OCI packaging, generic reference-hardware profile, local Apple-inspired dashboard

## v0.1.1 — Live Hardware

- background inventory refresh and material-change events
- scheduled sensor sampling + canonical sample envelopes
- real LM75/TMP102 I2C reference sensor
- plugin validation and bounded execution
- IP addresses + interface counters
- Nodra retained inventory/status, per-sensor and event topics
- live sensor/event UX

## v0.1.2 — Industrial bus health

- SocketCAN controller state, bitrate, CAN-FD mode and error telemetry
- passive RS485 declarations from config/device tree
- BUS-OFF doctor checks and Fleet/Nodra industrial status projection

## v0.1.3 — Industrial capture

- bounded RX-only SocketCAN capture on explicit interfaces
- raw CAN/CAN-FD SSE stream and Nodra hand-off
- no transmit API and no automatic bus reconfiguration
- J1939 semantics remain a Nodra connector

## v0.1.5 — Camera device discovery + live streaming

- [x] V4L2 device discovery, live JPEG snapshot, live MJPEG stream
  (`--features camera`, disabled by default — see `docs/CAMERA.md`) — the
  device-discovery and live-viewing half of the "Edge AI bridge" item
  below, deliberately scoped no further than that
- local inference event contract into Nodra, one accelerator family, and
  the Fleet health/application-lifecycle story for it remain **not**
  fully scoped — a `--features edge-ai` scaffold exposing
  `GET /api/v1/inference/events` (501 / `not-configured`) landed; see
  `docs/EDGE_AI.md` and the trimmed "Edge AI bridge" entry under "Later /
  not yet scoped"

## v0.2 — Production device identity

- [x] mTLS device certificates (v0.1.4 — client-side enrollment only; see
  `docs/MTLS_ENROLLMENT.md`)
- [x] secure bootstrap/enrollment token (v0.1.4)
- [x] TPM2 / secure-element identity when hardware provides it (v0.1.4 —
  `identity.backend = "tpm"`, `--features tpm2`; see `docs/TPM2_IDENTITY.md`)
- [x] Unix socket + local RBAC (v0.1.4)
- [x] privilege separation for sensor plugin subprocesses (v0.1.4 —
  `plugins.run_as_uid`/`run_as_gid`; the daemon itself intentionally still
  runs as root for direct GPIO/I2C/SPI/CAN bus access — see
  `docs/HARDWARE_PERMISSIONS.md`. A privilege-separation helper for the
  daemon's *own* bus access remains open; design + config/`--features privsep`
  scaffold landed — see `docs/PRIVSEP.md`, helper binary not implemented)
- [x] richer health thresholds and event sources (v0.1.4 — `[thresholds]`,
  thermal + CAN controller error counters; more event sources can still be
  added later)

## v0.3 — Nodra protocol packs

- Modbus RTU
- Modbus TCP
- CAN raw
- J1939
- OPC-UA
- BLE
- serial/custom binary
- LoRaWAN

These remain Nodra adapters, not Device Agent modules.

## Later / not yet scoped

Directionally plausible, but not committed to a version and not currently
being worked on as a full delivery. Also flagged in `docs/BACKLOG.md`'s
"explicitly outside Device Agent v0.x" list as out of scope for the v0.x
line specifically — these would need to be re-scoped (and that exclusion
revisited) before either becomes real roadmap work. Small scaffolds may
land ahead of that re-scope so clients can wire against stable paths:

- **OTA executor** — signed bundle verification, A/B inactive-slot write,
  reboot + health confirmation, commit/rollback, rollout initiated by Fleet.
  Device Agent can emit a signed `request-ota` remediation document only.
- **Edge AI bridge (remaining half)** — provider contract stubs land in
  `src/inference_provider.rs`; HTTP remains 501 until a real backend exists.
- **Daemon bus privsep helper** — implemented behind `--features privsep`
  and `[privsep].enabled`; stock root unit unchanged.

## v0.2.0 — Passport and flight recorder

Shipped: trust defaults, stream tickets, TPM policy, passport, recorder,
support bundles, commissioning installer, six-screen dashboard, signed
remediation, profile/plugin SDK surfaces, rules-based diagnostics.
Physical Minew certification remains an operator-signed QUALIFICATION row.