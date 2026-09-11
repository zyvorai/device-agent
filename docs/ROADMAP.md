# Roadmap

## v0.1 — Hardware Edge MVP

- ARM64 Linux first-class target; amd64 build for development
- inventory: identity, CPU, RAM, root storage, kernel, OS, thermal
- network: Ethernet/Wi-Fi/CAN discovery
- physical buses: GPIO, I2C, SPI, UART, CAN, USB, watchdog
- REST API + Prometheus endpoint
- external sensor plugin protocol
- Nodra MQTT publishing
- Fleet inventory projection
- systemd unit + OCI image
- Apple-inspired local dashboard
- Minewing reference profile and field acceptance test

## v0.2 — Production device identity

- mTLS device certificates
- secure bootstrap/enrollment token
- TPM2 / secure-element identity when hardware provides it
- Unix socket + local RBAC
- privilege separation helper for GPIO/I2C/SPI/CAN
- richer Prometheus collectors and event stream

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

## v0.4 — OTA executor

- signed bundle verification
- A/B inactive-slot write
- reboot + health confirmation
- commit/rollback
- rollout initiated by Fleet

## v0.5 — Edge AI bridge

- V4L2 / RTSP device discovery
- one accelerator family first
- local inference event contract into Nodra
- Fleet health and application lifecycle
