# Nodra Modbus RTU adapter contract

This file defines the integration seam for extending Nodra's existing `modbus` connector with an RTU transport. Nodra already has a dependency-free Modbus TCP client/poller; RTU should reuse the same connector/event path rather than create a second protocol stack. It is intentionally **not implemented inside Device Agent**.

## Responsibility split

```text
Device Agent
  "ttyS1 exists and this board declares it as RS485."

Nodra Modbus RTU adapter
  "slave 7, holding register 40021 is motor temperature = 82.3 C."
```

Device Agent supplies discovery from:

```http
GET http://127.0.0.1:9188/api/v1/industrial/serial
```

Nodra should then open only a configured serial port.

## Proposed Nodra RTU transport configuration

```json
{
  "connectors": [
    {
      "type": "modbus",
      "name": "line-1-plc",
      "config": {
        "transport": "rtu",
        "serial": {
          "port": "/dev/ttyS1",
          "baud": 9600,
          "data_bits": 8,
          "parity": "even",
          "stop_bits": 1,
          "timeout_ms": 750,
          "inter_frame_delay_ms": 5
        },
        "polls": [
          {
            "unit": 7,
            "function": "holding-registers",
            "address": 20,
            "count": 2,
            "every_ms": 1000
          }
        ],
        "topic": "factory/line-1/plc-7/telemetry"
      }
    }
  ]
}
```

## Adapter behavior

The Nodra transport extension should:

1. verify the selected port is visible in Device Agent when Device Agent is available;
2. never scan Modbus unit IDs by default;
3. enforce one serial transaction at a time per physical port;
4. implement Modbus RTU CRC16 and strict frame-length validation;
5. preserve existing `0x03` holding-register behavior, add `0x04` input-register reads, and keep existing TCP `0x06` writes gated behind an explicit write capability for RTU;
6. publish original sample time and polling latency;
7. classify timeout, CRC, exception response and framing errors separately;
8. write successful telemetry into Nodra's normal offline WAL path;
9. expose adapter health without making Device Agent responsible for protocol health;
10. back off on repeated failures rather than hammering a disconnected PLC.

## Event example

```json
{
  "adapter": "line-1-plc",
  "unit": 7,
  "event_time": "2026-09-11T13:30:00.123Z",
  "latency_ms": 18,
  "values": {
    "motor_temperature": 82.3,
    "motor_rpm": 1480
  }
}
```

## Safety

Writes (`0x05`, `0x06`, `0x0F`, `0x10`) should be a separate explicit
capability and disabled by default. The first Minewing demo only needs reads.


## Compatibility with the current Nodra connector

The current Nodra `connectors/modbus` package uses Modbus TCP framing and registers the connector type as `modbus`. Nodra agent configuration wraps connector-specific settings under `connectors[].config`. RTU should therefore be a `transport` choice under the same connector type. The polling loop should continue emitting `connector.Event` with the configured topic so Nodra's local routing, offline WAL, and downstream handling stay unchanged.
