---
hero:
  eyebrow: INDUSTRIAL ACCEPTANCE
  title: Industrial acceptance — v0.1.2
---

Run on the selected ARM64 board after installing Device Agent.

## 1. Hardware visibility

```bash
zyvor-device-agent inventory
zyvor-device-agent industrial
zyvor-device-agent doctor
```

Acceptance:

- expected Ethernet interface is present;
- expected CAN interfaces are present;
- configured CAN interface reports the expected bitrate after board setup;
- no CAN interface is `BUS-OFF`;
- expected UART is present;
- the selected RS485 port is explicitly declared;
- watchdog and the real I2C temperature sensor pass existing checks.

## 2. Configure the board, not the agent

Device Agent observes CAN. Board provisioning remains responsible for bringing
it up, for example:

```bash
ip link set can0 down
ip link set can0 type can bitrate 500000 restart-ms 100
ip link set can0 up
```

The exact bitrate must come from the attached industrial network.

Do not make Device Agent silently change CAN bitrate on startup.

## 3. RS485

Once the exact board UART is known, declare it:

```toml
[industrial]
rs485_ports = ["ttyS1"]
```

This is a physical-board declaration, not Modbus configuration.

## 4. Nodra hand-off

Nodra's Modbus RTU adapter should consume `/dev/ttyS1`, poll one known device
and publish through Nodra. Disconnect WAN during the test. Local polling and
WAL should continue; reconnect and verify buffered telemetry is forwarded.

## 5. Demo pass condition

```text
Reference ARM64
  ├─ I2C temperature       PASS
  ├─ CAN health/bitrate    PASS
  ├─ RS485 declaration     PASS
  ├─ Device Agent UI       PASS
  ├─ Nodra Modbus RTU      next Nodra implementation
  ├─ WAN loss              local path remains alive
  └─ Fleet projection      CAN/RS485 capability visible
```
