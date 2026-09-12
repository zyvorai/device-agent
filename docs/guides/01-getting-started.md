# 1. Getting started

This is the from-source path: clone the repo, build the daemon and the
dashboard, and see live hardware inventory on screen in a few minutes. If
you're deploying a packaged build to a remote Linux host instead of building
locally, skip ahead to
[6. Deploying to production](06-deploying-to-production.md) or
[`docs/QUICKSTART.md`](../QUICKSTART.md).

## Prerequisites

- Rust 1.85+ (`rustc --version`)
- Node.js + npm, only if you want to build the dashboard
- Linux for full hardware discovery (GPIO/I2C/SPI/CAN/etc.); on macOS/other
  platforms the daemon still runs, but hardware sections of the inventory
  will mostly be empty — that's expected for local development

## 1. Copy a config

```bash
cp config/device-agent.example.toml /tmp/device-agent.toml
```

The example config ships with `auth.mode = "none"` and binds to
`0.0.0.0:9188`. That's fine for local development on a machine you trust;
see [3. Securing your agent](03-securing-your-agent.md) before exposing this
to a network you don't.

## 2. Look at what the daemon sees, without starting a server

```bash
cargo run -- --config /tmp/device-agent.toml inventory
```

This prints the full hardware inventory as one JSON document and exits —
CPU/RAM/storage/OS/kernel, network interfaces, detected buses, and
industrial state. It's the same data the `serve` command publishes over
HTTP, useful for scripting or a quick sanity check without a running
daemon.

Two related one-shot commands:

```bash
cargo run -- --config /tmp/device-agent.toml doctor       # health/diagnostics report; exits non-zero if a check fails
cargo run -- --config /tmp/device-agent.toml industrial   # CAN + serial/RS485 hardware state only
```

## 3. Start the daemon

```bash
cargo run -- --config /tmp/device-agent.toml serve
```

You should see log lines confirming the HTTP listener came up and the
background inventory refresh loop started. Leave this running in one
terminal.

In another terminal, confirm it's alive:

```bash
curl -sS http://127.0.0.1:9188/api/v1/health      # always 200 once the process is up
curl -sS http://127.0.0.1:9188/api/v1/ready       # 200 once the first inventory refresh has ticked
curl -sS http://127.0.0.1:9188/api/v1/inventory   # the same JSON `inventory` printed above
```

## 4. Build and open the dashboard

The dashboard is a React/TypeScript/Vite app under `web/dashboard`, served
by the same daemon at the same port once built:

```bash
cd web/dashboard
npm ci
npm test
npm run build
```

Restart `serve` (it picks up `server.dashboard_dir` from config, pointed at
`web/dashboard/dist` by default in dev) and open
`http://127.0.0.1:9188/` in a browser. You should land on the **Overview**
page: device identity, health, CPU/RAM/temperature, and detected physical
interfaces. The other pages — **Hardware · Interfaces · Industrial ·
Sensors · Integrations · Diagnostics · Settings** — are listed in the nav
on the left.

## 5. Try the reference sensor plugin

```bash
python3 examples/i2c_temperature.py --self-test
```

This decodes a canned LM75/TMP102 register value without touching real
hardware — a quick way to confirm the plugin contract before wiring up
[5. Writing a sensor plugin](05-writing-a-sensor-plugin.md) against a real
I²C bus.

## Next steps

- [2. Configuration & the API](02-configuration-and-api.md) — what's in
  `device-agent.toml` and what each REST endpoint returns.
- [3. Securing your agent](03-securing-your-agent.md) — before you bind to
  anything other than loopback.
- The README's [Documentation](https://github.com/zyvorai/device-agent#documentation) section for
  the full reference-doc index.
