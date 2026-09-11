# Engineering Backlog

## v0.1.1 — Live hardware increment

- [x] background inventory refresh + cache
- [x] material hardware-change event stream (SSE)
- [x] bounded recent event history
- [x] actual IP-address discovery through `ip -j address`
- [x] RX/TX/error counters from sysfs
- [x] scheduled sensor sampling
- [x] canonical typed sensor sample envelope
- [x] per-plugin poll interval + Nodra publish flag
- [x] plugin absolute-path/executable/world-writable validation
- [x] plugin timeout + stdout size limit
- [x] real LM75/TMP102 I2C temperature reference plugin
- [x] retained Nodra inventory/status topics
- [x] per-sensor + event MQTT topics
- [x] Fleet address projection
- [x] expanded Prometheus metrics
- [x] live Sensors + event Diagnostics UX
- [x] install script scaffold

## P0 — Physical Minewing acceptance

- [ ] Select exact Minewing SKU and freeze expected device-tree nodes.
- [ ] Install the reference plugin on the physical unit and confirm the real I2C bus/address.
- [ ] Add the first RS485/Modbus RTU adapter in `zyvorai/nodra` (not Device Agent).
- [ ] Teach the existing `fleet-agent` to merge `/api/v1/integrations/fleet/inventory`.
- [ ] Hardware-in-loop test on the physical Minewing unit.
- [ ] WAN-loss/reconnect demo with Nodra WAL replay.

## P1 — Production hardening

- [ ] Local bearer/mTLS protection when binding beyond loopback.
- [ ] Unix-domain-socket API mode.
- [ ] Linux capability/udev policy documentation per bus.
- [ ] Plugin executable owner allowlist and process resource limits.
- [ ] Signed release artifacts, SBOM, provenance and cosign workflow.
- [ ] Debian/RPM packages and clean uninstall path.
- [ ] ARM64 smoke test under QEMU in CI.
- [ ] Optional Linux udev/netlink event source to complement polling.
- [ ] Configurable thermal/counter health thresholds and alert events.

## Explicitly outside Device Agent v0.x

Kubernetes dependency, industrial protocol meaning, Nodra WAL duplication, a
second Fleet agent, arbitrary remote shell, bootloader/partition OTA logic or
EdgeAI inference.
