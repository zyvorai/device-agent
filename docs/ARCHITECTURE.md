---
hero:
  eyebrow: ARCHITECTURE
  title: Architecture
  lead: >-
    The full security-model picture, and what's still open before GA.
  highlights:
    - {value: "9", label: "CLI subcommands in one binary", footnote: "1"}
    - {value: "3", label: "Auth modes shipped by v0.1.5 — none, bearer, mTLS"}
    - {value: "3", label: "Downstream planes — Nodra, Fleet, Aether"}
    - {value: "2", label: "mTLS key backends — software or TPM2"}
footnotes:
  - marker: "1"
    text: "serve, inventory, industrial, doctor, fleet-inventory, plugins, sample, enroll, identity."
    href: "#processes"
    href_label: "See Processes, above."
---

## Responsibility boundary

```text
Linux / reference board
        |
        v
Zyvor Device Agent
  - identity + inventory
  - Linux health
  - GPIO/I2C/SPI/UART/CAN discovery
  - local hardware API
  - sensor plugin process contract
  - Nodra publisher
  - Fleet inventory bridge
        |
        +----------------------+----------------------+
        |                      |                      |
        v                      v                      v
      Nodra                  Fleet                  Aether
 protocol/data plane     fleet/control plane   application/runtime plane
```

The Device Agent must not interpret industrial application protocols. It reports that `can0`, `/dev/i2c-1`, or `/dev/ttyS2` exist and provides safe local primitives. Nodra owns protocol semantics such as Modbus registers, J1939 PGNs, OPC-UA nodes, BLE profiles and store-and-forward behavior.

Aether may later use the Device Agent as a hardware capability source when deciding whether an application can run on a specific edge node. Aether is not a dependency of the Device Agent.

## Processes

`zyvor-device-agent` is one Rust binary with these CLI subcommands:

- `serve` — REST API + dashboard + Nodra/Fleet background workers
- `inventory` — one-shot JSON hardware inventory
- `industrial` — one-shot CAN/serial-RS485 hardware state
- `doctor` — diagnostics suitable for manufacturing and field support
- `fleet-inventory` — explicit Fleet inventory projection
- `plugins` — list configured sensor plugins and their validation state
- `sample <name>` — execute one configured sensor plugin immediately
- `enroll` — mTLS client enrollment: generate a CSR, submit it, persist the
  issued certificate/key (`auth.mode = "mtls"`) — see `docs/MTLS_ENROLLMENT.md`
- `identity` — print the current mTLS identity's subject, validity and key
  backend (`software` or `tpm`)

Sensor plugins are separate executables described by JSON manifests under `plugins.d`. This avoids loading third-party code into the long-running daemon and lets a plugin be implemented in Rust, Go, C, Python or shell for prototypes.

## API v1

See the README's "API" table for the full, current route list (health/ready/status,
inventory, industrial/CAN, plugins/sensors, events, doctor, metrics) — kept in one place
rather than duplicated here, since it changes with nearly every milestone.

## Security model

v0.1 is intended to bind to a trusted management LAN or localhost. As of v0.1.5: bearer-token
and mTLS API auth (`auth.mode = "bearer" | "mtls"`, the latter via client-side CSR
enrollment — see `docs/MTLS_ENROLLMENT.md`, optionally backed by a TPM2 via
`identity.backend = "tpm"` — see `docs/TPM2_IDENTITY.md`), Unix-socket mode with
peer-credential RBAC, opt-in CORS, per-plugin execution policy (owner/directory allowlists,
resource limits, an opt-in seccomp-bpf denylist) and opt-in privilege separation for plugin
subprocesses are done — see `docs/BACKLOG.md`. Transport encryption is a separate,
composable control: `server.tls.enabled` (v0.1.5) terminates plain TLS on the TCP listener
with no client certificate ever required — self-signed automatically on first start if no
cert is mounted — independent of `auth.mode`, so `server.tls.enabled = true` plus
`auth.mode = "bearer"` gives an encrypted transport with a required token without the
enrollment flow `auth.mode = "mtls"` needs. Still open before GA: signed Fleet inventory
bridge tokens, and privilege separation for the *daemon's own* physical bus access (it
intentionally still runs as root — see
`docs/HARDWARE_PERMISSIONS.md`).

## Take a closer look

=== "Industrial buses"

    Device Agent owns **physical-bus visibility**, not industrial protocol
    meaning. `GET /api/v1/industrial/can` always reads Linux sysfs for
    SocketCAN interface state and counters, and — when
    `industrial.can_ip_command` is set (default: `ip`) — a read-only
    `ip -j -details -statistics` query for controller state, bitrate,
    CAN-FD data bitrate and error counters. No CAN frames are opened,
    injected or decoded. RS485 is never guessed from a generic UART: it
    appears only when declared in `[industrial].rs485_ports` or present in
    the Linux device tree. See [Industrial buses](INDUSTRIAL_BUSES.md)
    and [4. Industrial buses](guides/04-industrial-buses.md).

=== "Camera"

    `--features camera` (off by default) adds live snapshot and MJPEG
    streaming from an explicitly allowlisted `/dev/video*` — the
    device-discovery and live-viewing half of the "Edge AI bridge" line
    item in [`ROADMAP.md`](ROADMAP.md); local inference stays a
    separate, still-unscoped increment. Acquisition only: no control
    tuning, no daemon-side recording, and only the single newest frame per
    camera is cached. `publish_to_nodra` sends health/presence only, never
    frame bytes. See [`docs/CAMERA.md`](CAMERA.md).

=== "Security & identity"

    As of v0.1.5: `auth.mode` is `none` (default), `bearer` (SHA-256 hash
    only, never the raw token), or `mtls` (client-side CSR enrollment via
    `enroll`, optionally backed by a TPM2 through
    `identity.backend = "tpm"`, falling back to software if the TPM can't
    be opened). `server.tls.enabled` is a separate, composable control —
    plain TLS with no client certificate required, self-signed on first
    start. A same-host Unix socket with peer-credential allowlists
    (`allow_uids`/`allow_gids`) bypasses both. See
    [`docs/MTLS_ENROLLMENT.md`](MTLS_ENROLLMENT.md) and
    [`docs/TPM2_IDENTITY.md`](TPM2_IDENTITY.md).
