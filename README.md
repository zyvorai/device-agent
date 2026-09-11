# Zyvor Device Agent

> Linux hardware edge agent for Zyvor — discover the box, expose physical interfaces, publish to Nodra, expose Fleet-compatible inventory.

**Status:** v0.1.1 live-hardware development · **License:** Apache-2.0 · **Targets:** Linux `arm64` first, `amd64` for development and CI.

```text
Minewing / Linux edge hardware
            |
            v
     Zyvor Device Agent
 identity · health · GPIO · I2C · SPI · UART · CAN · USB · watchdog
            |
       +----+-------------------+
       |                        |
       v                        v
     Nodra                    Fleet
 protocol + data plane      control plane
       |
 Modbus / J1939 / OPC-UA
 BLE / serial / LoRaWAN
```

## Why this exists

Zyvor already has higher layers for workload/runtime control and fleet/data-plane responsibilities. What was missing was a small Linux-native hardware foundation that can run directly on an ARM64 gateway without Kubernetes. Device Agent provides that boundary.

It intentionally does **not** interpret Modbus registers, CAN/J1939 PGNs or OPC-UA nodes. It reports physical capabilities and gives sensor drivers a stable local contract. Nodra owns industrial protocol semantics, routing and offline store-and-forward. Fleet owns remote lifecycle and desired state. Aether can later consume the node capability model but is not required on the device.

## v0.1 scope

- ARM64 Linux first-class support
- CPU / RAM / root storage / OS / kernel / uptime / temperature inventory
- Ethernet / Wi-Fi / CAN network discovery
- GPIO / I2C / SPI / UART / CAN / USB / watchdog discovery
- continuous cached inventory refresh + material hardware-change events
- sensor plugin process API + scheduled sampling
- real LM75/TMP102 I²C temperature reference plugin (explicit bus/address; no scanning)
- REST API
- Prometheus metrics endpoint
- Nodra MQTT publishing: retained inventory/status, per-sensor and event topics
- Fleet inventory bridge with hardware metadata and IP addresses
- systemd service
- OCI image with `linux/amd64` + `linux/arm64` CI
- local Apple-inspired dashboard using the same React/Vite family as Aether
- Minewing reference-board profile + acceptance test

## Quick start

```bash
cp config/device-agent.example.toml /tmp/device-agent.toml
cargo run -- --config /tmp/device-agent.toml inventory
cargo run -- --config /tmp/device-agent.toml doctor
cargo run -- --config /tmp/device-agent.toml serve

# Reference sensor decode test; does not touch hardware
python3 examples/i2c_temperature.py --self-test
```

Dashboard/API: `http://127.0.0.1:9188`

Build the dashboard:

```bash
cd web/dashboard
npm ci
npm test
npm run build
```

## API

| Endpoint | Purpose |
|---|---|
| `GET /api/v1/health` | daemon liveness and version |
| `GET /api/v1/status` | agent generation/sample counters |
| `GET /api/v1/inventory` | cached full device inventory |
| `POST /api/v1/inventory/refresh` | force an immediate Linux inventory refresh |
| `GET /api/v1/interfaces` | network, buses and USB |
| `GET /api/v1/thermal` | Linux thermal zones |
| `GET /api/v1/integrations` | Nodra/Fleet connection state |
| `GET /api/v1/plugins` | plugin manifests + validation state |
| `POST /api/v1/plugins/{name}/sample` | execute one plugin sample |
| `GET /api/v1/sensors` | latest canonical samples |
| `GET /api/v1/sensors/{sensor_id}` | latest sample for one sensor |
| `GET /api/v1/events` | live Server-Sent Events stream |
| `GET /api/v1/events/recent` | bounded recent event history |
| `GET /api/v1/doctor` | field diagnostics |
| `GET /metrics` | Prometheus text exposition |

## Product boundary

```text
Device Agent: "There is a CAN interface named can0."
Nodra:        "0x18FF50E5 is engine temperature = 82°C."
Fleet:        "Apply config X to device ZY-MW-0001 and restart workload Y."
Aether:       "This application requires CAN + 4 cores; this node is eligible."
```

This separation is a design rule, not just an implementation detail.

## UX principles

The dashboard is a **local hardware cockpit**. It opens on one screen showing device identity, health, CPU/RAM/temperature, detected physical interfaces, and Nodra/Fleet status. The visual language is intentionally minimal: system typography, white space, glass-like navigation, monochrome surfaces, dark diagnostics, and Zyvor orange used only for emphasis.

Pages: **Overview · Hardware · Interfaces · Sensors · Integrations · Diagnostics · Settings**.

## Repository map

```text
src/                    Rust daemon
  hardware/             Linux/sysfs discovery only
  integrations/         Nodra and Fleet adapters
  api.rs                 REST API
  plugins.rs             external sensor plugin contract
web/dashboard/           React + TypeScript + Vite UX
config/                  runtime configuration
packaging/systemd/       Linux service
examples/                plugin examples
scripts/                 packaging helpers
docs/                    architecture, roadmap, Minewing profile
.github/workflows/       CI and tagged release pipeline
```

## First demo acceptance test

A clean ARM64 Minewing unit must be able to: install one Zyvor package → start the agent → auto-detect hardware → read one real sensor through a plugin → publish through Nodra → keep working during WAN loss → sync after reconnect through Nodra WAL → appear in Fleet through the existing fleet-agent → expose health for remote lifecycle operations.

See `docs/MINEWING_REFERENCE.md`, `docs/V0.1.1_LIVE_HARDWARE.md` and `docs/ROADMAP.md`.

## License

Apache License 2.0. See `LICENSE` and `NOTICE`.
