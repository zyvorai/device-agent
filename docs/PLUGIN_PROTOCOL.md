# Sensor Plugin Protocol v1

A plugin is an executable plus a JSON manifest. Device Agent launches the
command directly (never through a shell) with `ZYVOR_PLUGIN_PROTOCOL=v1` and
expects one JSON object on stdout.

## Single-reading form

```json
{
  "sensor": "cabinet-temperature",
  "value": 31.2,
  "kind": "temperature",
  "unit": "celsius",
  "quality": "good",
  "labels": { "bus": "/dev/i2c-1", "address": "0x48" }
}
```

The daemon wraps that response in a canonical envelope containing the plugin
name, sensor ID, collection time, success state, normalized readings and raw
plugin output. The canonical envelope is used by REST, the dashboard and Nodra.

A plugin may instead return a `readings` array when one transaction produces
multiple measurements:

```json
{
  "sensor": "environment-1",
  "quality": "good",
  "readings": [
    { "name": "temperature", "kind": "temperature", "value": 31.2, "unit": "celsius" },
    { "name": "humidity", "kind": "humidity", "value": 62.1, "unit": "percent" }
  ]
}
```

## Manifest

```json
{
  "name": "i2c-temperature",
  "version": "0.1.1",
  "command": "/usr/lib/zyvor-device-agent/plugins/i2c_temperature.py",
  "args": ["--bus", "/dev/i2c-1", "--address", "0x48"],
  "capabilities": ["temperature", "i2c"],
  "sensor_id": "cabinet-temperature",
  "enabled": true,
  "poll_interval_seconds": 5,
  "publish_to_nodra": true
}
```

## Safety rules

By default Device Agent requires an absolute plugin command, rejects commands
that are not executable regular files, rejects world-writable executables and
caps captured stdout. A hung plugin is killed after the configured timeout.

Device Agent does **not** scan I2C addresses automatically. Some I2C devices
react badly to probing. The included LM75/TMP102 reference plugin only touches
the bus and address explicitly configured in its manifest.

The process boundary is deliberate. High-rate protocols belong in Nodra
adapters; Device Agent plugins are for bounded hardware/sensor sampling.
