# 4. Industrial buses

Device Agent reports the *presence and health* of CAN and serial/RS485
buses. It deliberately never decodes protocol payloads (Modbus registers,
J1939 PGNs, OPC-UA nodes) — that's Nodra's job. This guide covers the
hardware-facing half; the protocol-level hand-off docs are linked at the
end.

## CAN health, read-only

No configuration needed to see it — SocketCAN interfaces are discovered
automatically from Linux sysfs, enriched with optional `ip -j -details
-statistics` output:

```bash
cargo run -- --config /path industrial
curl -sS http://127.0.0.1:9188/api/v1/industrial/can | jq .
```

You'll get `BUS-OFF` state, nominal bitrate, CAN-FD data bitrate,
controller error counters, netdevice counters, driver, and
physical-vs-virtual classification, per interface. Device Agent never opens
or decodes CAN frames for this — it only reads controller state.

```toml
[industrial]
can_ip_command = "ip"
```

If [thresholds] are enabled (see the README's "Configurable health
thresholds" section), CAN error counters feed
`threshold.breached`/`threshold.recovered` events at the CAN
error-warning/error-passive levels defined by the CAN spec (96/128 by
default).

## RS485 declaration

RS485 discovery is passive — Device Agent will not probe a serial port to
guess what's attached. Declare it explicitly, or let Linux device-tree
properties identify it:

```toml
[industrial]
rs485_ports = ["ttyS1"]
publish_to_nodra = true
```

## Turning on read-only CAN frame capture

Disabled by default. When enabled, only the explicitly allowlisted
interfaces are opened on a receive-only raw socket — there is no transmit
endpoint, and capture never changes bitrate, CAN-FD mode, restart policy or
controller state:

```toml
[industrial.can_capture]
enabled = true
interfaces = ["can0"]
history_limit = 512
max_frames_per_second = 200
include_error_frames = false
publish_to_nodra = true
```

Restart the daemon (capture sockets are opened once at startup — a
`SIGHUP` reload won't pick up a change here), then:

```bash
curl -sS http://127.0.0.1:9188/api/v1/can/capture | jq .          # status/counters
curl -sS http://127.0.0.1:9188/api/v1/can/frames/recent | jq .    # bounded history
curl -sSN http://127.0.0.1:9188/api/v1/can/frames/stream          # live SSE
```

Classic, extended, and CAN-FD frames (with BRS/ESI flags) all show up here,
and — if `publish_to_nodra = true` — on per-interface Nodra topics too.
Frames are rate-limited per interface and kept in a bounded in-memory
history; nothing is persisted to disk.

## Where protocol decoding happens

Device Agent hands off raw bus access to Nodra's own connectors rather than
decoding payloads itself:

- [`INDUSTRIAL_BUSES.md`](../INDUSTRIAL_BUSES.md) — the full CAN/RS485
  hardware-layer reference.
- [`CAN_CAPTURE.md`](../CAN_CAPTURE.md) — capture internals and safety
  rules in depth.
- [`NODRA_J1939_HANDOFF.md`](../NODRA_J1939_HANDOFF.md) — how captured CAN
  frames reach Nodra's `j1939-device-agent` connector for PGN decoding.
- [`NODRA_MODBUS_RTU_CONTRACT.md`](../NODRA_MODBUS_RTU_CONTRACT.md) — the
  proposed adapter seam for RS485/Modbus RTU.

## Next steps

- [5. Writing a sensor plugin](05-writing-a-sensor-plugin.md) if what you
  actually need is a bounded, polled sensor reading rather than raw bus
  access.
- [`INDUSTRIAL_ACCEPTANCE.md`](../INDUSTRIAL_ACCEPTANCE.md) for the
  end-to-end acceptance checklist this feeds into.
