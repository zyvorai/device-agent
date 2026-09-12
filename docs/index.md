# Zyvor Device Agent

Linux hardware edge agent for Zyvor — discover the box, expose physical
interfaces, publish to Nodra, expose Fleet-compatible inventory.

## Why this exists

Zyvor already has higher layers for workload/runtime control and
fleet/data-plane responsibilities. What was missing was a small
Linux-native hardware foundation that can run directly on an ARM64
gateway without Kubernetes. Device Agent provides that boundary.

It intentionally does **not** interpret Modbus registers, CAN/J1939 PGNs
or OPC-UA nodes. It reports physical capabilities and gives sensor
drivers a stable local contract. Nodra owns industrial protocol
semantics, routing and offline store-and-forward. Fleet owns remote
lifecycle and desired state.

For the full picture — quick start, REST API reference, product
boundary, repository map and license — see the
[**README on GitHub**](https://github.com/zyvorai/device-agent#readme).

## Start here

- [Tutorials — guides index](guides/README.md) — a numbered series from
  first build to production/container deployment
- [FAQ](FAQ.md) — licensing, support, production-readiness, cloud/data
  questions for anyone still evaluating
- [Troubleshooting](TROUBLESHOOTING.md) — real issues people hit, with
  the actual fix
- [Reference hardware](REFERENCE_HARDWARE.md) — the capability profile a
  board needs, not a fixed SKU list
- [Architecture](ARCHITECTURE.md) — the full security-model picture and
  what's still open before GA
