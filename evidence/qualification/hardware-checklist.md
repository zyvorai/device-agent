# Device Agent hardware qualification checklist

SKU / revision: Minewing GW1 / r1  
Profile: `minewing-gw1-r1`  
Image / package digest: ________________  
Operator: ________________  Date: __________

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
