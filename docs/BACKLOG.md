# v0.1 Engineering Backlog

## P0 — Milestone acceptance

- [x] Repository skeleton, Apache-2.0, NOTICE, security policy
- [x] ARM64-first Rust daemon architecture
- [x] Linux identity / CPU / RAM / root-storage / OS / kernel / uptime discovery
- [x] thermal-zone discovery
- [x] Ethernet / Wi-Fi / CAN network discovery
- [x] GPIO / I2C / SPI / UART / CAN / USB / watchdog enumeration
- [x] REST API and Prometheus endpoint
- [x] external sensor-plugin manifest/process contract
- [x] Nodra MQTT telemetry publisher
- [x] Fleet-compatible local inventory projection
- [x] systemd unit and multi-stage OCI Dockerfile
- [x] Apple-inspired React/Vite local hardware cockpit
- [x] Minewing ARM64 board profile and profile-aware `doctor`
- [ ] Select exact Minewing SKU and freeze expected device-tree nodes
- [ ] Implement first real I2C temperature sensor plugin
- [ ] Implement first RS485/Modbus RTU Nodra adapter in `zyvorai/nodra`
- [ ] Teach existing `fleet-agent` to merge `/api/v1/integrations/fleet/inventory`
- [ ] Hardware-in-loop test on the physical Minewing unit
- [ ] WAN-loss/reconnect demo with Nodra WAL replay

## P1 — Before v0.1.0 release

- [ ] local bearer/mTLS protection when binding beyond loopback
- [ ] Unix-domain-socket API mode
- [ ] Linux capability/udev policy documentation per bus
- [ ] structured event stream for hot-plug and health transitions
- [ ] actual IP-address inventory (not just interface inventory)
- [ ] sensor sampling scheduler + typed sample envelope
- [ ] per-plugin allowlist, executable ownership/mode validation and resource limits
- [ ] dashboard Sensors page backed by live plugin samples
- [ ] signed release artifacts, SBOM, provenance and cosign workflow
- [ ] Debian/RPM packages and install/uninstall scripts
- [ ] ARM64 smoke test under QEMU in CI

## Explicitly out of v0.1

Kubernetes dependency, OTA partition writing, bootloader logic, EdgeAI inference, generic remote shell, industrial protocol semantics inside Device Agent, or replacing Fleet/Nodra agents.
