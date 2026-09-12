# 2. Configuration & the API

Once the daemon is running (see
[1. Getting started](01-getting-started.md)), this guide walks through the
config file section by section and shows how the dashboard's pages map onto
the REST API underneath them.

## The config file

Start from `config/device-agent.example.toml` — every section is commented
in place, so treat it as the primary reference, not this guide. The
top-level sections you'll touch most:

| Section | Controls |
|---|---|
| `[server]` | Listen address, dashboard static-file directory |
| `[server.unix_socket]` | Additional same-host API listener, peer-uid/gid allowlisted |
| `[server.cors]` / `[server.rate_limit]` | Cross-origin access and per-IP request limits (TCP listener only) |
| `[auth]` | `"none"` / `"bearer"` / `"mtls"` — see [3. Securing your agent](03-securing-your-agent.md) |
| `[industrial]` | CAN `ip` command path, RS485 port declarations, Nodra publishing |
| `[industrial.can_capture]` | Opt-in read-only CAN frame capture — see [4. Industrial buses](04-industrial-buses.md) |
| `[plugins]` | Manifest directory (`plugins.d`) and optional sandboxing — see [5. Writing a sensor plugin](05-writing-a-sensor-plugin.md) |
| `[thresholds]` | Optional thermal/CAN-error alerting thresholds |
| `[nodra]` / `[fleet]` | Upstream MQTT publishing and Fleet inventory bridge |

A config is loaded once at startup. Most sections can be re-applied live
with `SIGHUP` without restarting — see
[6. Deploying to production](06-deploying-to-production.md) for exactly
which fields need a full restart instead.

`cargo run -- --config /path doctor` is the fastest way to check whether a
config is valid and the daemon can see what it expects before you commit to
`serve`.

## The REST API

Every route below is served on `server.listen` (`http://127.0.0.1:9188` in
the default config). If `auth.mode` isn't `"none"`, most routes need
credentials — see [3. Securing your agent](03-securing-your-agent.md).

| Endpoint | Purpose | Where it shows up in the dashboard |
|---|---|---|
| `GET /api/v1/health` | Liveness — always 200 once the process is up | — (used by orchestration/systemd, not the UI) |
| `GET /api/v1/ready` | Readiness — 200 once the inventory loop is ticking | — |
| `GET /api/v1/status` | Generation/sample counters | Diagnostics |
| `GET /api/v1/inventory` (alias `/api/v1/hardware`) | Full cached device inventory | Overview, Hardware |
| `POST /api/v1/inventory/refresh` | Force an immediate refresh | Settings |
| `GET /api/v1/interfaces` | Network, buses, industrial state, USB | Interfaces |
| `GET /api/v1/industrial` / `/industrial/can` / `/industrial/serial` | CAN + serial/RS485 state | Industrial |
| `GET /api/v1/can/capture` / `/can/frames/recent` / `/can/frames/stream` | Read-only CAN capture status/history/live stream | Industrial |
| `GET /api/v1/thermal` | Linux thermal zones | Overview, Diagnostics |
| `GET /api/v1/integrations` / `/integrations/fleet/inventory` | Nodra/Fleet connection state and inventory projection | Integrations |
| `GET /api/v1/plugins` | Plugin manifests + validation state | Sensors |
| `POST /api/v1/plugins/{name}/sample` | Execute one plugin sample | Sensors |
| `GET /api/v1/sensors` / `/sensors/{sensor_id}` | Latest canonical sensor samples | Sensors |
| `GET /api/v1/events` / `/events/recent` | Live/recent event stream | Diagnostics |
| `GET /api/v1/doctor` | Field diagnostics | Diagnostics |
| `GET /metrics` | Prometheus text exposition | — (scraped, not shown in-app) |

Try a few directly:

```bash
curl -sS http://127.0.0.1:9188/api/v1/interfaces | jq .
curl -sS http://127.0.0.1:9188/api/v1/sensors | jq .
curl -sS -X POST http://127.0.0.1:9188/api/v1/inventory/refresh
```

The two Server-Sent Events streams (`/api/v1/events`,
`/api/v1/can/frames/stream`) can be watched from the shell too:

```bash
curl -sSN http://127.0.0.1:9188/api/v1/events
```

## Next steps

- [3. Securing your agent](03-securing-your-agent.md) once you're ready to
  bind beyond loopback.
- [4. Industrial buses](04-industrial-buses.md) for the `[industrial]`
  section in depth.
- Full field-by-field detail beyond what's covered here: the comments in
  `config/device-agent.example.toml` itself are the source of truth.
