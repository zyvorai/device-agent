---
hero:
  eyebrow: PRODUCTION
  title: Production operations runbook — Zyvor Device Agent
---

Companion to [guides/06-deploying-to-production.md](guides/06-deploying-to-production.md)
and [guides/03-securing-your-agent.md](guides/03-securing-your-agent.md).

## Current maturity (2026-09-14)

| Claim | Status |
|---|---|
| Software matrix + emulator HIL CI | green (`hil-ci-emulator`, packages amd64/arm64) |
| Hardened Linux agent packages | shippable (`v0.1.6`) with auth/TLS guidance below |
| Minewing GW1 r1 physical HIL | **unsigned** — lab host is x86 surrogate (`minewing_claimable=false`) |
| Lab-surrogate HIL evidence | recorded (`evidence/qualification/hil/`) — `minewing_claimable=false` |
| Hardware checklist | **not signed** — do not claim Minewing GA |

**Verdict:** device-agent is **production-ready as a hardened Linux agent** on supported
arches when auth/TLS (or UDS) are configured. It is **not** Minewing-silicon-qualified
until [HIL.md](HIL.md) is signed on real hardware.

## Preconditions

1. `make qualify` green → `evidence/qualification/software-matrix.json`.
2. Hardware checklist signed for the SKU (bring-up: **Minewing GW1 r1**).
3. `auth.mode` is `bearer` or `mtls` when binding beyond loopback (or use UDS only).
4. Prefer `server.tls.enabled = true` (or terminate TLS at a frontier).
5. Nodra: enable `[nodra.tls]` unless MQTT is strictly on a trusted LAN/loopback.
6. Arm64 install via release **`.deb`/`.rpm`**, **tarball**, or GHCR image.

## Day-2 monitoring

- Scrape `/metrics` (authenticate when bearer/mTLS is on).
- Alert on `zyvor_device_agent_nodra_connected == 0` when Nodra is enabled.
- Watch inventory generation stalls and plugin sample gaps.

## OTA Mark-good contract

Zyvor OTA health checks should include at least:

```json
[
  {"kind": "systemd", "target": "zyvor-device-agent.service"},
  {"kind": "http", "target": "http://127.0.0.1:9188/api/v1/health"}
]
```

Use HTTPS loopback if the agent API is TLS-only. See
[`profiles/minewing-gw1-r1.md`](../profiles/minewing-gw1-r1.md).

## Nodra MQTTS

```toml
[nodra]
enabled = true
broker = "nodrad.example"
port = 8883

[nodra.tls]
enabled = true
ca_file = "/etc/zyvor/mqtt-ca.pem"
# cert_file / key_file optional for broker mTLS
```

Plain MQTT (`tls.enabled = false`) is **trusted LAN / loopback only**.

## Verify after deploy

```bash
./scripts/verify-deployment.sh HOST USER
ZYVOR_DEVICE_AGENT_TLS=1 ./scripts/verify-deployment.sh HOST USER
ZYVOR_DEVICE_AGENT_BEARER_TOKEN=… ./scripts/verify-deployment.sh HOST USER
```

## Needs attention (known limits)

- Daemon unit still typically runs as root; plugins can drop privileges.
- Partial `SIGHUP` reload — Nodra reconnect / listen bind need restart.
- arm64 `.deb`/`.rpm` shipped via release matrix on `ubuntu-24.04-arm`.
- Physical HIL still open for GA claims.
