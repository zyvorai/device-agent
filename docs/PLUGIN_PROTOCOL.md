# Sensor Plugin Protocol v1

A plugin is an executable plus a JSON manifest. The daemon launches the command with `ZYVOR_PLUGIN_PROTOCOL=v1` and expects one JSON object on stdout.

Example:

```json
{
  "sensor": "cabinet-temperature",
  "value": 31.2,
  "unit": "celsius",
  "timestamp": "2026-09-11T12:00:00Z",
  "quality": "good",
  "labels": { "bus": "i2c-1", "address": "0x48" }
}
```

v0.1 deliberately uses a process boundary. A future high-rate plugin API can add Unix sockets or a local gRPC stream without changing the public REST inventory model.
