---
hero:
  eyebrow: EDGE AI
  title: 'Edge AI bridge: inference event contract (scaffold)'
---

Camera discovery and live MJPEG (`--features camera`, see
[`CAMERA.md`](CAMERA.md)) shipped the device-viewing half of the "Edge AI
bridge" roadmap item. This document covers the **remaining half**: a local
inference event contract into Nodra, one accelerator family, and Fleet
health/application lifecycle for it.

**Status:** scaffold only. Build with `--features edge-ai` to expose the
placeholder HTTP surface. No NPU/GPU runtime is linked, no model is loaded,
and no frames are consumed for inference yet.

## Safety contract

- off by default at both compile time (`edge-ai` feature) and runtime
  (`[edge_ai].enabled`)
- without a real accelerator backend, every inference route returns
  **HTTP 501** with `error = "not-configured"` — never a silent empty stream
- Device Agent still does not warehouse video; inference (when it lands) will
  publish **event envelopes**, never raw frames, matching camera's Nodra
  boundary
- choosing an `accelerator` family string in config does not load a driver

## Config

```toml
[edge_ai]
# Scaffold: never starts a backend even when true. See docs/EDGE_AI.md.
enabled = false
# Placeholder family name, e.g. "rknn", "openvino", "tensorrt". Empty = undeclared.
accelerator = ""
# Future: publish inference event envelopes to Nodra (never frame bytes).
publish_to_nodra = true
```

`[edge_ai]` is always parsed so configs stay stable across builds. The
`/api/v1/inference/*` routes exist only when the binary is built with
`--features edge-ai`.

## Endpoints

```text
GET /api/v1/inference/events    # placeholder; always 501 until a backend lands
```

Example 501 body (`contract_version` bumps when successful event payloads
change shape):

```json
{
  "error": "not-configured",
  "message": "Edge AI inference is not configured: no accelerator backend is enabled",
  "contract_version": 1,
  "configured": false,
  "accelerator": null,
  "publish_to_nodra": true
}
```

A future success path is expected to look like SSE (same family as
`/api/v1/events` and `/api/v1/can/frames/stream`) carrying typed inference
results — detection labels, scores, timestamps — not JPEG bytes.

## Nodra / Fleet (not implemented)

Planned, not wired:

- retained accelerator health/presence topic (mirrors camera status)
- non-retained inference event topic when `publish_to_nodra = true`
- Fleet inventory projection of accelerator readiness / application lifecycle

## What is deliberately still out of scope

- picking and shipping one accelerator SDK
- model packaging / OTA of weights
- RTSP camera discovery
- running inference inside the privileged bus path (see [`PRIVSEP.md`](PRIVSEP.md)
  if inference ever needs camera frames from a de-privileged helper)

See `docs/ROADMAP.md` ("Later / not yet scoped") for how this scaffold sits
relative to a committed version.
