# Device Agent hardware qualification checklist

**Status:** harness ready (`scripts/hil/run-minewing-hil.sh`); **unsigned for Minewing
silicon**. Lab surrogate run recorded under `evidence/qualification/hil/` —
does **not** close production HIL claims.

| Test | Pass? | Notes |
|---|---|---|
| `doctor` profile checks green on board | | |
| Ethernet / UART / CAN / USB / watchdog present | | |
| I2C reference plugin reads real sensor | | |
| Nodra MQTT (or MQTTS) connected | | |
| Fleet inventory projection reachable | | |
| Auth ≠ none when non-loopback | | |
| OTA health probes (systemd + /health) | | |
| WAN-loss / reconnect with Nodra WAL | | |

**Production claim:** software qualify passed; this checklist is signed for the
exact build above.

Signature: ______________________
