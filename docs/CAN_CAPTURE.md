# Read-only CAN capture

v0.1.3 adds an intentionally narrow SocketCAN receive path for diagnostics and Nodra hand-off.

## Safety contract

- disabled by default
- explicit interface allowlist only
- receive-only raw CAN socket; Device Agent has no transmit endpoint
- never changes bitrate, CAN-FD mode, restart policy or controller state
- bounded in-memory history
- per-interface accepted-frame rate cap
- CAN error frames excluded unless explicitly enabled
- standard CAN, extended CAN and CAN-FD metadata preserved

```toml
[industrial.can_capture]
enabled = true
interfaces = ["can0"]
history_limit = 512
max_frames_per_second = 200
include_error_frames = false
publish_to_nodra = true
```

Endpoints:

```text
GET /api/v1/can/capture
GET /api/v1/can/frames/recent
GET /api/v1/can/frames/stream   # SSE, event name can.frame
```

Nodra topic when publishing is enabled:

```text
<topic-prefix>/<serial>/industrial/can/<interface>/frames
```

Each payload preserves the numeric CAN identifier, whether it is extended/RTR/error/CAN-FD,
CAN-FD BRS/ESI flags, DLC, byte array and hexadecimal data. Device Agent does not compute
J1939 PGNs; that semantic translation belongs in Nodra.
