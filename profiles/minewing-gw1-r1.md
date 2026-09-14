# Minewing GW1 r1 profile

Bring-up SKU for Zyvor Device Agent, aligned with Zyvor OTA
`boards/minewing-gw1-r1` (`compatible = minewing-gw1-r1`).

## Software expectations

Set in `/etc/zyvor/device-agent.toml`:

```toml
[device]
vendor = "Minewing"
model = "GW1"
profile = "minewing-gw1-r1"
```

`zyvor-device-agent doctor` evaluates the `[minimum]` bus counts in
`profiles/minewing-gw1-r1.toml` against live sysfs inventory.

## OTA health contract

Zyvor OTA Mark-good probes commonly include:

| Kind | Target |
|---|---|
| `systemd` | `zyvor-device-agent.service` |
| `http` | `http://127.0.0.1:9188/api/v1/health` (or TLS equivalent) |

Do not mark an OS good until the agent is active and `/api/v1/health` returns 200.

## Hardware rows

Physical I2C plugin path, CAN HIL, and power-loss drills are **not** closed by
this profile file. Sign `evidence/qualification/hardware-checklist.md`.
