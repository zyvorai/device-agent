---
hero:
  eyebrow: NODRA J1939 HANDOFF
  title: Nodra J1939 hand-off
---

Device Agent v0.1.3 exposes raw CAN frames without interpreting them. Nodra owns the protocol layer.

Recommended local path:

```text
SocketCAN can0
   -> Zyvor Device Agent RX-only capture
   -> /api/v1/can/frames/stream
   -> Nodra j1939-device-agent connector
   -> PGN/source/destination decode
   -> Nodra durable event/WAL
   -> local route / cloud / Relay
```

The Nodra companion overlay registers connector type `j1939-device-agent`.

Example `nodrad.json` connector:

```json
{
  "type": "j1939-device-agent",
  "name": "vehicle-can",
  "config": {
    "url": "http://127.0.0.1:9188/api/v1/can/frames/stream",
    "topic_prefix": "factory/line-1/j1939",
    "reconnect": "2s"
  }
}
```

Only extended 29-bit data frames are translated. Standard, RTR and CAN error frames are ignored by
the J1939 connector. PDU1 identifiers zero the destination byte when computing the PGN, while
preserving destination separately.

## Avoiding duplicate raw ingest

When the `j1939-device-agent` SSE connector is enabled, set `industrial.can_capture.publish_to_nodra = false` if you only want decoded J1939 events in Nodra. Leave it `true` when you intentionally want both the raw CAN archive topic and decoded PGN topics.
