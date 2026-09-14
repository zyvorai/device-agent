---
hero:
  eyebrow: QUALIFICATION
  title: Production qualification matrix — Zyvor Device Agent
---

Software rows are automated by `make qualify`. Physical Minewing GW1 r1 HIL
is driven by [`scripts/hil/run-minewing-hil.sh`](https://github.com/zyvorai/device-agent/blob/main/scripts/hil/run-minewing-hil.sh)
([HIL.md](HIL.md)). Lab-surrogate and `hil-ci-emulator` evidence **never**
set `minewing_claimable=true`. Hardware checklist remains unsigned until
physical silicon passes.

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

## Lab substitute / surrogate (not Minewing)

| Path | Evidence | Claim |
|---|---|---|
| CI `hil-ci-emulator` | `evidence/qualification/ci/` | green substitute; `minewing_claimable=false` |
| Lab-surrogate HIL | `evidence/qualification/hil/20260914T162450Z/` | wiring on x86 host; `minewing_claimable=false` |
| Physical Minewing | `hardware-checklist.md` | **unsigned** |

## Operator / hardware rows — checklist status

| Test | Lab / CI status |
|---|---|
| Profile freeze on Minewing board | **open** (surrogate ≠ silicon) |
| I2C plugin path on board | **open** |
| Industrial buses on board | **open** |
| Auth + TLS / UDS on agent | **done** in software + packages v0.1.6 |
| Nodra MQTTS config | **done** in software matrix |
| OTA health probe contract | **documented** — see PRODUCTION.md |
| Arm64 install artifacts | **ship** — release + CI packages |

## Arm64 packaging posture

| Artifact | amd64 | arm64 |
|---|---|---|
| `.deb` / `.rpm` | Release + CI `packages-amd64` | Release + CI `packages-arm64` (`ubuntu-24.04-arm`) |
| Release tarball | yes | yes (`linux-arm64`) |
| GHCR OCI | yes | yes |

Native arm64 packages are built on GitHub's `ubuntu-24.04-arm` runners (not
cross-packed) so architecture metadata and `$auto` depends stay correct.

## Maturity note

**v0.1.6** is **production-hardening** for the Linux agent (packages + auth/TLS
guidance + emulator/lab-surrogate evidence). It is **not** Minewing-silicon GA.
See [PRODUCTION.md](PRODUCTION.md), [HIL.md](HIL.md), and [BACKLOG.md](BACKLOG.md).
