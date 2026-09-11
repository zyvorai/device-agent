# Minewing Reference Profile

The first board profile is intentionally declarative. Exact device nodes vary by Minewing SKU and BSP, so the profile should be finalized from the selected board's device tree and Linux image.

## MVP acceptance profile

```yaml
vendor: Minewing
arch: arm64
required:
  ethernet: 1
optional:
  wifi: true
  gpio: true
  i2c: 1
  spi: 1
  uart: 1
  can: 1
  usb: true
  watchdog: true
```

## Manufacturing / field test

1. Boot a clean ARM64 Linux image.
2. Install the Device Agent package.
3. `zyvor-device-agent inventory` must identify CPU, RAM, OS, network and physical buses.
4. `zyvor-device-agent doctor` must exit 0.
5. Start the service and open `http://DEVICE:9188`.
6. Enable one real I2C temperature plugin.
7. Publish that sample to Nodra.
8. Disconnect WAN; local UI and Nodra ingestion remain operational.
9. Reconnect WAN; Nodra flushes its WAL upstream.
10. Enroll with Fleet and verify remote health/application lifecycle.

The Device Agent does not implement the offline WAL; that remains a Nodra responsibility.
