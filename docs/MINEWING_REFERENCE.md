# Minewing Reference Profile

The first board profile remains declarative because exact device-tree aliases and
device nodes depend on the selected Minewing SKU and BSP. v0.1.1 adds a real
I2C sensor path without pretending that all Minewing boards expose identical
bus numbering.

## MVP acceptance profile

```yaml
vendor: Minewing
arch: arm64
required:
  ethernet: 1
  i2c: 1
  uart: 1
  can: 1
recommended:
  gpio: true
  spi: true
  usb: true
  watchdog: true
```

## Reference temperature path

```text
LM75 / TMP102
     |
     | explicit /dev/i2c-N + 0xNN
     v
examples/i2c_temperature.py
     |
     v
Device Agent SensorSample
     |
     +--> local REST / dashboard / Prometheus health
     |
     +--> Nodra MQTT -> local routes / WAL / upstream
```

No active I2C scan is performed. Confirm the selected board schematic, BSP and
sensor address before enabling the installed manifest.

## Manufacturing / field test

1. Boot a clean ARM64 Linux image.
2. Install Device Agent with `scripts/install.sh` or the future native package.
3. `zyvor-device-agent inventory` must identify CPU, RAM, OS, IP/network and physical buses.
4. `zyvor-device-agent doctor` must pass the selected board profile.
5. Start the service and open `http://DEVICE:9188` on the management network.
6. Confirm `/dev/i2c-N` and the wired sensor address, then enable the reference plugin.
7. `zyvor-device-agent sample i2c-temperature` must return a valid temperature envelope.
8. Verify live samples/events in the local cockpit.
9. Enable Nodra and verify inventory, status and per-sensor topics.
10. Disconnect WAN; local sampling and Nodra local ingestion must continue.
11. Reconnect WAN; Nodra flushes its own WAL upstream.
12. Enroll with Fleet and verify projected hardware/address inventory and application lifecycle.
