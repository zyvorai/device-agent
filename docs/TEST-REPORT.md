---
hero:
  eyebrow: TEST REPORT
  title: Verification report — Zyvor Device Agent v0.1.5
---

Recorded for the production-hardening line. This is not Minewing hardware
certification.

## What CI / `make qualify` establish

| Check | Result |
|---|---|
| `cargo fmt` / clippy `-D warnings` / `cargo test --all` | Required software gate |
| Optional feature legs | `hotplug`, `tpm2`, `camera` in CI |
| Emulator smokes | `emulator-vcan`, `emulator-swtpm`; `emulator-v4l2` best-effort — [EMULATOR_CI.md](EMULATOR_CI.md) |
| ARM64 cross-build + QEMU container smoke | CI `rust-arm64` / container jobs |
| Nodra MQTTS config (`[nodra.tls]`) | Unit-tested transport builder |
| Minewing GW1 r1 profile file | Present under `profiles/` |
| Physical RAUC / power-loss / real I2C HIL | **Not run** — hardware checklist |
| arm64 native `.deb`/`.rpm` | CI `packages-arm64` + release matrix on `ubuntu-24.04-arm` |

## Lab wiring (optional evidence)

A multi-product lab co-located Device Agent with Nodra MQTT and Fleet inventory
merge (see sibling `docs/LAB.md` in Fleet/OTA). That proves networking and
inventory projection; it does not close hardware rows.

## Reproduce

```bash
make check
make qualify
# optional on Linux CI-like hosts:
make emulator-vcan emulator-swtpm
make emulator-v4l2   # soft-skip if no v4l2loopback
```

Sign [`evidence/qualification/hardware-checklist.md`](../evidence/qualification/hardware-checklist.md)
via [`scripts/hil/run-minewing-hil.sh`](../scripts/hil/run-minewing-hil.sh)
([HIL.md](HIL.md)) before production claims on a physical SKU.
