---
hero:
  eyebrow: FAQ
  title: FAQ
---

Questions people evaluating Device Agent actually ask, before they've
decided to adopt it. If you're already building with it, the
[tutorial guides](guides/README.md) are a better starting point.

## Licensing & cost

**Is it really free?** Yes. The daemon, dashboard, and everything in this
repository are Apache-2.0 — free to use, modify, and run for personal, lab,
and commercial production use at no charge, subject to preserving notices
(see [`NOTICE`](https://github.com/zyvorai/device-agent/blob/main/NOTICE)). See the README's [License](https://github.com/zyvorai/device-agent#license)
section.

**What does "Enterprise" mean, then?** Production support, SLAs, and
Zyvor's other commercial products are licensed separately from this
open-source agent. Contact sales@zyvor.dev or see zyvor.dev if you need
that; nothing in this repository requires it.

## Support

**What if I find a bug?** Open a GitHub issue on this repository.

**What if I find a security vulnerability?** Don't open a public issue —
see [`SECURITY.md`](https://github.com/zyvorai/device-agent/blob/main/SECURITY.md) for private reporting instructions and
the current v0.1 security boundary list.

**Is there a support SLA?** Not for the open-source project itself —
community support is via GitHub issues. Contact sales@zyvor.dev for
commercial support arrangements.

## Production readiness

**Is this production-ready?** Status is **v0.1.6 production-hardening** — not
GA, and **not** Minewing-silicon-qualified. The Linux agent (packages,
auth/TLS, emulator/`hil-ci-emulator`, lab-surrogate) is shippable for
hardened installs; physical Minewing HIL remains unsigned
(`minewing_claimable=false` on surrogate evidence). Run `make qualify`
and sign the hardware checklist only after physical HIL. See
[PRODUCTION.md](PRODUCTION.md), [HIL.md](HIL.md), and
[TEST-REPORT.md](TEST-REPORT.md). [ROADMAP.md](ROADMAP.md) lists what is
still ahead (e.g. local AI inference remains unscoped).

**How stable is the API?** Each REST endpoint and config field added is
documented in the README under its introducing version (e.g. "v0.1.4"). No
formal API-stability guarantee is made pre-1.0 — check `CHANGELOG.md` when
upgrading.

**Does Nodra MQTT need TLS?** Prefer `[nodra.tls] enabled = true` (MQTTS)
whenever the path leaves a trusted LAN. Plain MQTT is loopback/LAN-only.

## Cloud & data

**Does this require a cloud account or internet connection to run?** No.
Device Agent runs entirely on the device — inventory, health, REST API,
dashboard and Prometheus metrics all work with zero network dependency
beyond serving those locally. Publishing to Nodra (industrial protocol
routing) and projecting into Fleet (remote lifecycle) are both optional
integrations, off unless configured.

**What happens if the network drops?** The documented acceptance criterion
(README's "First demo acceptance test") is explicit: the agent must "keep
working during WAN loss" and "sync after reconnect through Nodra WAL" — WAN
connectivity loss doesn't stop local hardware monitoring or health
reporting; only upstream publishing queues until reconnect.

**Where does my data go?** Nowhere, by default. Nothing leaves the device
unless you configure Nodra MQTT publishing or Fleet's inventory bridge —
both point at endpoints you control, not a Zyvor-operated cloud.

## Security

**What's the security model?** See [`SECURITY.md`](https://github.com/zyvorai/device-agent/blob/main/SECURITY.md) in
full. In short: the daemon is intended for a trusted Linux edge node,
plugin manifests must be administrator-provisioned (no arbitrary remote
code execution), and the REST API supports bearer-token auth, mTLS (with
optional TPM2-backed key storage), and independent transport TLS — see the
README's "API auth and Unix socket", "mTLS enrollment", and "TLS" sections.

**Does it operate its own certificate authority?** No — `enroll` is a
client submitting a CSR to an enrollment endpoint you run; key custody,
revocation and rotation are your organization's responsibility. See
[`docs/MTLS_ENROLLMENT.md`](MTLS_ENROLLMENT.md).

## Extensibility

**Can I add my own sensors?** Yes — the sensor plugin protocol
([`docs/PLUGIN_PROTOCOL.md`](PLUGIN_PROTOCOL.md)) is a documented contract:
an executable plus a JSON manifest, run out-of-process, with optional
privilege-drop and seccomp sandboxing.

**Can I add my own camera/video processing?** Device Agent captures and
serves live snapshot/MJPEG streams ([`docs/CAMERA.md`](CAMERA.md)); it
deliberately does not run inference on frames itself — that's future,
still-unscoped work per `docs/ROADMAP.md`'s "Edge AI bridge" entry.

## Hardware

**What hardware is supported?** See the README's
["Is this for you?"](https://github.com/zyvorai/device-agent#is-this-for-you) section and
[`docs/REFERENCE_HARDWARE.md`](REFERENCE_HARDWARE.md) — a capability
profile (required buses + recommended buses), not a fixed list of
certified boards.

**Does it manage a fleet of devices?** No — Device Agent runs one instance
per device and projects its inventory/health into Zyvor Fleet, which owns
fleet-wide lifecycle and desired-state management. Device Agent itself has
no multi-device concept.

## Platform

**What OS/architecture does it run on?** Linux only. arm64 is first-class;
amd64 works for development, CI, and containers. No Windows or macOS
support (macOS can build and run the daemon for development, but hardware
discovery returns mostly empty results there — see
[1. Getting started](guides/01-getting-started.md)).
