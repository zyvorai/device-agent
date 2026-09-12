---
hero:
  eyebrow: GUIDES
  title: 8. Camera streaming
---

Live snapshot and live video from a USB or CSI camera, following the same
opt-in, explicitly-allowlisted posture as
[4. Industrial buses](04-industrial-buses.md)'s CAN capture. This is the
device-discovery and live-viewing half of the "Edge AI bridge" line item in
[`ROADMAP.md`](../ROADMAP.md) — local inference and hardware-accelerator
support are a separate, still-unscoped future increment, not covered here.

## 1. Build with the `camera` feature

Off by default, like `hotplug`/`tpm2`:

```bash
cargo build --release --features camera
```

`v4l` (pure-Rust V4L2 ioctls) and `jpeg-encoder` (pure-Rust JPEG encoder,
used only for cameras that don't offer native MJPG) compile in only with
this flag — a plain `cargo build` is unaffected. V4L2 is Linux-only; on any
other platform the capture code compiles to a no-op stub, same as
`hardware::hotplug`.

## 2. Declare a camera

No camera is ever opened unless explicitly listed — same rule as CAN
capture's interface allowlist:

```toml
[camera]
devices = []

[[camera.devices]]
id = "front-dock"
path = "/dev/video0"
enabled = true
max_frames_per_second = 10
jpeg_quality = 75
max_stream_clients = 4
publish_to_nodra = true
```

Restart the daemon after changing this — camera capture threads are opened
once at startup, same as CAN capture's interfaces.

## 3. Take a live snapshot

```bash
curl -sS -o snapshot.jpg http://127.0.0.1:9188/api/v1/camera/front-dock/snapshot
```

Returns the single most recently captured frame as `image/jpeg`. `404` if
`front-dock` isn't a configured camera id; `503` if it's configured but
hasn't captured a frame yet (not yet enabled, still starting, or the
capture thread is currently erroring — check `GET /api/v1/camera` for
`last_error`).

## 4. View the live stream

```bash
curl -sS http://127.0.0.1:9188/api/v1/camera/front-dock/stream -o /dev/null
```

Or, simplest of all, point a browser or a plain `<img>` tag straight at it:

```html
<img src="http://127.0.0.1:9188/api/v1/camera/front-dock/stream">
```

This is `multipart/x-mixed-replace` — the same shape a classic IP camera
serves — so it renders as a live-updating image with no WebRTC/signaling
setup. Capture runs exactly once regardless of how many viewers connect;
`max_stream_clients` (`0` = unlimited) caps concurrent viewers with a `429`
past the limit.

If `auth.mode = "bearer"` is set (see
[3. Securing your agent](03-securing-your-agent.md)), an `<img src>` can't
set an `Authorization` header — append the token as a query parameter
instead, same fallback the two SSE routes already use:

```html
<img src="https://<host>:9188/api/v1/camera/front-dock/stream?token=<TOKEN>">
```

## 5. Check status

```bash
curl -sS http://127.0.0.1:9188/api/v1/camera | jq .
```

Returns, per configured camera: `capturing`, `frames_total`,
`dropped_total` (rate-limit drops), `encode_errors_total`, `subscribers`
(current live viewers), and `last_error`.

## 6. Nodra hand-off (optional)

`publish_to_nodra = true` publishes only **health/presence** —
capturing state and the counters above — to a retained
`<topic-prefix>/<serial>/camera/<id>/status` topic. Raw frame bytes never
reach Nodra; this is deliberate, matching the product boundary described in
the main [README](https://github.com/zyvorai/device-agent#product-boundary) — Device Agent reports
capabilities, Nodra owns higher-level semantics.

## Next steps

- [`docs/CAMERA.md`](../CAMERA.md) — full config/endpoint reference and the
  safety contract.
- [`docs/HARDWARE_PERMISSIONS.md`](../HARDWARE_PERMISSIONS.md) — the
  `/dev/video*` row, relevant if the daemon is ever run as non-root.
- [`docs/ROADMAP.md`](../ROADMAP.md) — where local inference/accelerator
  support fits once it's scoped.
