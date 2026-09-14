---
hero:
  eyebrow: EMULATOR CI
  title: Emulator CI — vcan, swtpm, v4l2loopback
---

Software qualification covers three **emulator** paths so CI can exercise
industrial/camera/TPM code without Minewing silicon. Physical HIL remains
[`evidence/qualification/hardware-checklist.md`](../evidence/qualification/hardware-checklist.md).

## Scripts

| Script | What it proves |
|---|---|
| `scripts/emulator/smoke-vcan.sh` | Read-only CAN capture receives a frame on `vcan0` |
| `scripts/emulator/smoke-swtpm.sh` | TPM identity generate → persist JSON envelope → reload → sign |
| `scripts/emulator/smoke-v4l2.sh` | `--features camera` snapshot against `v4l2loopback` |
| `scripts/emulator/smoke-all.sh` | Runs all three (respects `DA_EMULATOR_STRICT`) |

```bash
make emulator-vcan      # STRICT=1 by default
make emulator-swtpm
make emulator-v4l2     # STRICT=0 by default (module may be missing)
make emulator
```

## CI jobs

- `emulator-vcan` — required; installs `can-utils`, loads `vcan`.
- `emulator-swtpm` — required; installs `swtpm` + `libtss2-dev`, runs
  `identity::tpm::live_swtpm_tests` with `ZYVOR_SWTPM_TCTI`.
- `emulator-v4l2` — best-effort (`continue-on-error`); needs
  `v4l2loopback-dkms` matching the runner kernel. Soft-skip is acceptable.

`make qualify` in CI marks the three rows via `DA_EMULATOR_VCAN` /
`DA_EMULATOR_SWTPM` / `DA_EMULATOR_V4L2`.

## Host packages

| Emulator | Typical apt packages |
|---|---|
| vcan | `iproute2`, `can-utils` |
| swtpm | `swtpm`, `swtpm-tools`, `tpm2-tools`, `libtss2-dev` |
| v4l2 | `v4l2loopback-dkms`, `linux-headers-$(uname -r)`, `ffmpeg`, `libclang-dev` |
