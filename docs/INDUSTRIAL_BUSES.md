# Industrial buses — Device Agent v0.1.2

Device Agent owns **physical-bus visibility**, not industrial protocol meaning.

```text
Linux / device tree / sysfs
          │
          ▼
  Zyvor Device Agent
  ├─ CAN interface state
  ├─ bitrate / CAN-FD data bitrate when `ip` can report it
  ├─ controller + netdevice error counters
  ├─ physical vs virtual CAN
  ├─ UART/USB serial inventory
  └─ passive RS485 declaration
          │
          ▼
         Nodra
  ├─ Modbus RTU
  ├─ J1939
  ├─ custom serial protocol
  └─ semantic telemetry / WAL / twins
```

## CAN

`GET /api/v1/industrial/can` returns a record for each SocketCAN interface.
The agent always reads Linux sysfs for interface state and counters. When
`industrial.can_ip_command` is non-empty (default: `ip`), it also performs a
read-only `ip -j -details -statistics link show dev <name>` query to obtain
controller state, nominal bitrate, CAN-FD data bitrate, restart timeout and
controller error counters.

No CAN frames are opened, injected or decoded by Device Agent.

Example:

```json
{
  "name": "can0",
  "kind": "physical",
  "operstate": "up",
  "driver": "m_can_platform",
  "bitrate": 500000,
  "data_bitrate": 2000000,
  "can_state": "ERROR-ACTIVE",
  "tx_error_counter": 0,
  "rx_error_counter": 0,
  "rx_errors": 0,
  "tx_errors": 0,
  "controller_modes": ["fd"],
  "details_source": "iproute2+sysfs"
}
```

`BUS-OFF` is surfaced as a failing `doctor` check. The agent does not attempt
an automatic reset because restart policy belongs to the board/operator.

## Serial and RS485

Serial discovery is passive. The daemon lists `/dev/ttyS*`, `/dev/ttyAMA*`,
`/dev/ttyUSB*` and `/dev/ttyACM*`, resolves their kernel driver when possible,
and classifies SoC vs USB transport.

RS485 is **not guessed** from a generic UART. It appears only when either:

1. the port is declared in `[industrial].rs485_ports`; or
2. Linux device-tree RS485 properties are present for that TTY.

Example configuration:

```toml
[industrial]
can_ip_command = "ip"
rs485_ports = ["ttyS1", "/dev/ttyUSB0"]
publish_to_nodra = true
```

The agent never opens every serial port or sends probe bytes. That prevents
discovery from disturbing PLCs, motor controllers or attached field devices.

## Nodra topics

When both Nodra and industrial publishing are enabled, Device Agent publishes
retained status on:

```text
<device-prefix>/industrial/status
<device-prefix>/industrial/can/<interface>/status
<device-prefix>/industrial/serial/<port>/status
```

These topics contain hardware state only. Protocol data should be emitted by
Nodra adapters on application-specific telemetry topics.

## Prometheus

v0.1.2 adds:

```text
zyvor_device_agent_rs485_declared_ports
zyvor_device_agent_can_up{interface="can0"}
zyvor_device_agent_can_bus_off{interface="can0"}
zyvor_device_agent_can_bitrate_bits_per_second{interface="can0"}
zyvor_device_agent_can_rx_errors_total{interface="can0"}
zyvor_device_agent_can_tx_errors_total{interface="can0"}
```
