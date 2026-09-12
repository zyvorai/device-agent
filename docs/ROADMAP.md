# Roadmap

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
  daemon's *own* bus access, as opposed to plugin subprocesses, remains open)
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
being worked on. Also flagged in `docs/BACKLOG.md`'s "explicitly outside
Device Agent v0.x" list as out of scope for the v0.x line specifically —
these would need to be re-scoped (and that exclusion revisited) before
either becomes real roadmap work:

- **OTA executor** — signed bundle verification, A/B inactive-slot write,
  reboot + health confirmation, commit/rollback, rollout initiated by Fleet.
- **Edge AI bridge** — V4L2/RTSP device discovery, one accelerator family
  first, local inference event contract into Nodra, Fleet health and
  application lifecycle.
