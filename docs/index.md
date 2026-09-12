---
hero:
  eyebrow: LINUX EDGE AGENT
  title: Zyvor Device Agent
  lead: >-
    Discover the box, expose physical interfaces, publish to Nodra, and
    expose Fleet-compatible inventory — a small Linux-native hardware
    foundation for an ARM64 edge gateway, with no Kubernetes required.
  highlights:
    - {value: "24", label: "REST API endpoints", footnote: "1"}
    - {value: "8", label: "Guided tutorials, first build to production"}
    - {value: "3", label: "Auth modes — none, bearer, mTLS"}
    - {value: "9", label: "CLI subcommands in one binary", footnote: "2"}
    - {value: "2", label: "CPU architectures — arm64 and amd64"}
  hub_bands:
    - icon: "🧭"
      title: Evaluating
      description: Questions people actually ask before adopting it — licensing, support, production-readiness, and cloud/data dependency.
      href: FAQ.md
    - icon: "📘"
      title: Guides
      description: Narrative, step-by-step walkthroughs, read in order the first time through — from first build to production and container deployment.
      href: guides/README.md
    - icon: "🛠️"
      title: Reference
      description: The capability profile a board needs, not a fixed SKU list — plus industrial-bus, security, camera, and plugin reference docs.
      href: REFERENCE_HARDWARE.md
    - icon: "🏗️"
      title: Architecture & planning
      description: The full security-model picture, and what's still open before GA.
      href: ARCHITECTURE.md
footnotes:
  - marker: "1"
    text: "Counted from the README's API table: health, ready, status, inventory (+ alias), refresh, interfaces, industrial (+ can, serial), CAN capture (+ recent, stream), thermal, integrations (+ fleet inventory), plugins (+ sample), sensors (+ by id), events (+ recent), doctor, and /metrics."
    href: "https://github.com/zyvorai/device-agent#api"
    href_label: "See the API table."
  - marker: "2"
    text: "serve, inventory, industrial, doctor, fleet-inventory, plugins, sample, enroll, identity."
    href: "ARCHITECTURE.md#processes"
    href_label: "See Processes."
---

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

## v0.1 scope, at a glance

<div class="icon-badge-list" markdown="1">

- 🔌 GPIO / I2C / SPI / UART / CAN / USB / watchdog discovery
- 🚌 Read-only SocketCAN health plus passive RS485 declaration
- 📼 Disabled-by-default, RX-only CAN frame capture on an explicit allowlist
- 🎥 Live camera snapshot + MJPEG streaming, off by default (`--features camera`)
- 🔐 Bearer, mTLS, and optional TPM2-backed identity
- 🧩 Out-of-process sensor plugins — Rust, Go, C, Python, or shell
- 📊 REST API plus a Prometheus metrics endpoint
- 📡 Nodra MQTT publishing and a Fleet inventory bridge

</div>

## Is this for you?

Device Agent is a small, single-purpose, open-source (Apache-2.0) hardware
layer for a Linux edge gateway. If you need a home-automation hub or a
full container fleet/OTA platform instead, one of these is probably a
closer fit; see the README's
[full comparison table](https://github.com/zyvorai/device-agent#is-this-for-you)
for AWS IoT Greengrass and Azure IoT Edge as well.

<div class="compare-cards" markdown="1">

- **Device Agent**

  Hardware inventory, health, and bounded sensor/bus access. No cloud
  dependency required — Nodra/Fleet integration is optional. Industrial
  protocol decoding and fleet/OTA orchestration are deliberately out of
  scope, handed to Nodra and Zyvor Fleet.

- **Home Assistant**

  Home automation hub, integrations, and local rules. Industrial fieldbus
  support, if any, comes through community integrations, not a built-in
  primitive.

- **balena**

  Fleet OS plus container deployment/OTA as its core feature, backed by
  balenaCloud (proprietary) for fleet management.

</div>
