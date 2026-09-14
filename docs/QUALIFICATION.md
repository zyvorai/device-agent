---
hero:
  eyebrow: QUALIFICATION
  title: Production qualification matrix — Zyvor Device Agent
---

Software rows are automated by `make qualify`. Physical Minewing GW1 r1 HIL
and bus acceptance remain operator-signed in
[`evidence/qualification/hardware-checklist.md`](../evidence/qualification/hardware-checklist.md).

## Software (host) rows — `make qualify`

| ID | Expected |
|---|---|
| `minewing_profile` | `profiles/minewing-gw1-r1.toml` present |
| `cargo_fmt` / `clippy_default` / `unit_tests` | Format, clippy `-D warnings`, `cargo test --all` |
| `nodra_mqtts_config` | `[nodra.tls]` deserializes; MQTTS transport builds |
| `emulator_vcan_ci` | `scripts/emulator/smoke-vcan.sh` (CI `emulator-vcan`) |
| `emulator_swtpm_ci` | `scripts/emulator/smoke-swtpm.sh` (CI `emulator-swtpm`) |
| `emulator_v4l2_ci` | `scripts/emulator/smoke-v4l2.sh` (CI `emulator-v4l2`; soft-skip OK) |

These prove agent software, Nodra MQTTS config, and **emulator** paths for
CAN / TPM2 / camera where the host can load the matching kernel/userspace
emulators. They **do not** prove GPIO/I2C/CAN on real silicon.

## Operator / hardware rows — signed checklist

| Test | Required outcome |
|---|---|
| Profile freeze | `device.profile = "minewing-gw1-r1"`; doctor profile checks pass on board |
| I2C plugin path | Reference temperature plugin reads real bus/address |
| Industrial buses | CAN/UART/RS485 presence per [INDUSTRIAL_ACCEPTANCE.md](INDUSTRIAL_ACCEPTANCE.md) |
| Auth + TLS | `auth.mode` ≠ `none` (or UDS-only); API TLS or loopback-only bind |
| Nodra path | Plain MQTT only on trusted LAN; else `[nodra.tls] enabled = true` |
| OTA health | `zyvor-device-agent.service` + `/api/v1/health` usable as Mark-good probes |
| Arm64 install | Tarball or multi-arch container (amd64 `.deb`/`.rpm` optional) |

## Arm64 packaging posture

| Artifact | amd64 | arm64 |
|---|---|---|
| `.deb` / `.rpm` | Release pipeline | **Not yet** — tracked backlog |
| Release tarball | yes | **yes** (`linux-arm64`) |
| GHCR OCI | yes | **yes** |

Production on arm64 gateways: prefer the signed arm64 tarball or multi-arch
container until native packages ship.

## Maturity note

v0.1.5 is **production-hardening**, not GA. See [TEST-REPORT.md](TEST-REPORT.md)
and [BACKLOG.md](BACKLOG.md).
