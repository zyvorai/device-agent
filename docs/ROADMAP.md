# Roadmap

## v0.1 — Hardware Edge MVP

- Linux hardware identity/inventory, physical buses and thermal discovery
- REST, Prometheus, external sensor-plugin process contract
- Nodra MQTT publishing and Fleet inventory projection
- systemd/OCI packaging, Minewing profile, local Apple-inspired dashboard

## v0.1.1 — Live Hardware

- background inventory refresh and material-change events
- scheduled sensor sampling + canonical sample envelopes
- real LM75/TMP102 I2C reference sensor
- plugin validation and bounded execution
- IP addresses + interface counters
- Nodra retained inventory/status, per-sensor and event topics
- live sensor/event UX

## v0.2 — Production device identity

- mTLS device certificates
- secure bootstrap/enrollment token
- TPM2 / secure-element identity when hardware provides it
- Unix socket + local RBAC
- privilege separation helper for GPIO/I2C/SPI/CAN
- richer health thresholds and event sources

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
