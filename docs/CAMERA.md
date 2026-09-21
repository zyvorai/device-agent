---
hero:
  eyebrow: CAMERA
  title: 'Camera: live snapshot + live MJPEG stream'
---

Starts the "Edge AI bridge" line item in `docs/ROADMAP.md`, scoped
deliberately to its device-discovery and live-viewing half — local
inference has a separate scaffold (`docs/EDGE_AI.md`, `--features edge-ai`)
and still has no accelerator backend. Requires the daemon to be built with
`--features camera` (off by default, like `hotplug`/`tpm2`).

## Safety contract

- disabled by default; the feature itself must be compiled in, and each
  camera must be individually `enabled = true`
- explicit device allowlist only — never auto-opens an undeclared
  `/dev/video*`, same posture as `[industrial.can_capture]`'s interface
  allowlist
- acquisition only: no exposure/white-balance/other `VIDIOC_S_CTRL` control
  tuning, no daemon-side recording/storage
- per-camera accepted-frame rate cap
- only the single newest frame per camera is cached — no video history is
  warehoused
- MJPG-capable cameras: the driver's own JPEG bytes are passed straight
  through. Other cameras: software YUYV→RGB→JPEG fallback (pure Rust, no
  `libjpeg`/`ffmpeg`/`gstreamer` dependency). A camera advertising neither
  format is not captured.
- on error, the capture thread retries with a bounded backoff (unlike CAN
  capture's "fail once" policy) — USB webcam replug is common enough to be
  worth recovering from automatically

## Config

```toml
[camera]
devices = []

# [[camera.devices]]
# id = "front-dock"
# path = "/dev/video0"
# enabled = true
# max_frames_per_second = 10
# jpeg_quality = 75       # 1-100, used only on the YUYV software-encode fallback path
# max_stream_clients = 4  # 0 = unlimited
# publish_to_nodra = true # health/presence only, see below
```

`id` is a stable identifier used in API paths — not the `/dev/videoN` path,
whose numbering isn't stable across reboots/replugs.

## Endpoints

```text
GET /api/v1/camera                    # status for every configured camera
GET /api/v1/camera/{id}/snapshot      # latest frame, image/jpeg
GET /api/v1/camera/{id}/stream        # multipart/x-mixed-replace MJPEG — works in a plain <img> tag
```

- `404`: `{id}` isn't a configured camera.
- `503`: configured but no frame captured yet (not enabled, still starting, or erroring).
- `429` on `/stream`: `max_stream_clients` exceeded.

`<img src>` can't set an `Authorization` header, so mint a short-lived
stream ticket with `POST /api/v1/stream-tickets` and pass `?ticket=` —
never the long-lived bearer. See `src/auth/tickets.rs`.

## Nodra hand-off

When `camera.devices[].publish_to_nodra = true`, only **health/presence**
is published — capturing state, frame/drop/encode-error counters, last
error — never frame bytes:

```text
<topic-prefix>/<serial>/camera/<id>/status   # retained
```

This is a deliberate boundary, not an oversight: raw video must never reach
Nodra in this increment. Device Agent's job stops at "here's a live JPEG
feed and camera health"; a future local-inference bridge (scaffold in
`docs/EDGE_AI.md`, `--features edge-ai`) or Nodra itself would own
actually consuming frames.

## Permissions

The daemon opens `/dev/video*` directly (not a plugin subprocess) and runs
as root by default, so no group membership is needed today — see
`docs/HARDWARE_PERMISSIONS.md`'s camera row for the `video`-group path if
the daemon is ever de-privileged.
