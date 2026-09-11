# Architecture

## Responsibility boundary

```text
Linux / Minewing board
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

`zyvor-device-agent` is one Rust binary with four CLI modes:

- `serve` — REST API + dashboard + Nodra/Fleet background workers
- `inventory` — one-shot JSON inventory
- `doctor` — diagnostics suitable for manufacturing and field support
- `enroll` — explicit Fleet inventory projection

Sensor plugins are separate executables described by JSON manifests under `plugins.d`. This avoids loading third-party code into the long-running daemon and lets a plugin be implemented in Rust, Go, C, Python or shell for prototypes.

## API v1

- `GET /api/v1/health`
- `GET /api/v1/inventory`
- `GET /api/v1/hardware`
- `GET /api/v1/interfaces`
- `GET /api/v1/thermal`
- `GET /api/v1/integrations`
- `GET /api/v1/plugins`
- `POST /api/v1/plugins/{name}/sample`
- `GET /api/v1/doctor`
- `GET /metrics`

## Security model

v0.1 is intended to bind to a trusted management LAN or localhost. Before GA, add mTLS/device identity, signed Fleet inventory bridge tokens, strict CORS, Unix-socket mode, per-plugin execution policy, seccomp profiles and optional privilege separation for physical bus access.
