# Changelog

## 0.1.2-dev — 2026-09-11

- Add industrial bus inventory without turning Device Agent into a protocol gateway.
- Add read-only SocketCAN controller state, bitrate/CAN-FD bitrate and error telemetry.
- Detect physical vs virtual CAN and surface BUS-OFF in `doctor`.
- Add passive serial/RS485 awareness from explicit config and Linux device-tree properties.
- Add `/api/v1/industrial`, `/industrial/can` and `/industrial/serial`.
- Add CAN/RS485 Prometheus metrics and Fleet metadata.
- Publish retained industrial status into Nodra MQTT topics.
- Add Apple-style Industrial cockpit page.
- Add Minewing industrial acceptance guide and Nodra Modbus RTU adapter contract.

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
