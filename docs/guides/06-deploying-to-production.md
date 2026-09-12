---
hero:
  eyebrow: GUIDES
  title: 6. Deploying to production
---

Three ways to get Device Agent onto a real Linux host, from most to least
automated, plus how to change its config live afterward and get it into
Prometheus.

## Option A: one-shot remote deploy (recommended for a single host)

```bash
./scripts/deploy-remote.sh user@host --auth-mode bearer
```

Builds the agent and dashboard **on the remote host** (nothing built
locally), installs the systemd unit, and — because of `--auth-mode
bearer` — generates a bearer token, hashes it, and prints it once:

```
==> Bearer token (generated · save this, it is not stored on the remote): <TOKEN>
==> /metrics requires the same bearer token. Prometheus scrape-config snippet:
```

**Save that token now.** Lost it? Reissue with
`FORCE_BEARER_TOKEN_RESET=1 ./scripts/deploy-remote.sh user@host --auth-mode bearer`.
Useful flags: `--quick` (skip toolchain install), `--no-ui` (skip the
dashboard build), `--bind ADDR`, `--dry-run`. `--auth-mode` only supports
`none`/`bearer` — for mTLS, deploy with `none`/`bearer` first, then follow
[3. Securing your agent](03-securing-your-agent.md)'s `mtls` section by
hand. Redeploys never touch an existing config file.

Confirm it's up:

```bash
ZYVOR_DEVICE_AGENT_BEARER_TOKEN=<TOKEN> ./scripts/verify-deployment.sh host user
```

This script (and `deploy-remote.sh`'s own built-in checks) only speak plain
`http://`. If the target has `server.tls.enabled = true` (see
[3. Securing your agent](03-securing-your-agent.md)), expect every check
past "service is active" to report failure — that's the daemon correctly
refusing plain HTTP, not a broken deploy. Verify by hand instead:
`curl -sSk https://host:9188/api/v1/health`.

## Option B: signed `.deb`/`.rpm` packages

Attached to every GitHub Release alongside the tarballs (amd64 only for
now — arm64 packages are tracked in `docs/BACKLOG.md`):

```bash
sudo dpkg -i zyvor-device-agent_*.deb   # or: sudo rpm -i zyvor-device-agent-*.rpm
sudo systemctl enable --now zyvor-device-agent
```

Same layout as `scripts/install.sh`
(`/usr/bin/zyvor-device-agent`, the systemd unit,
`/etc/zyvor/device-agent.toml`, profiles, the disabled-by-default I2C
plugin). The package **does not** enable or start the service for you —
review the config first, then `enable --now` yourself.
`/etc/zyvor/device-agent.toml` is a conffile: a locally-modified config
survives upgrades, and uninstalling leaves it and your plugin manifests in
place rather than deleting them.

## Option C: `scripts/install.sh` directly

What both of the above ultimately call — use it directly if you're
building your own packaging/provisioning around it rather than using
`deploy-remote.sh`'s SSH flow or the `.deb`/`.rpm` artifacts.

## Reload config without a restart

```bash
sudo systemctl kill -s HUP zyvor-device-agent
```

Re-reads the config file and applies `auth.*`, `thresholds.*`,
`plugins.*`, `fleet.*`, and most of `industrial.*`/`nodra.*` live.
Exceptions that still need a full restart: `server.listen`,
`server.unix_socket.*`, `server.dashboard_dir` (bound once at startup —
`SIGHUP` logs a warning and leaves them as-is), the Nodra MQTT connection
itself, and which interfaces `industrial.can_capture` has open. A
malformed config on `SIGHUP` is logged and ignored — the daemon keeps
running on its last-known-good config rather than crashing.

## Scrape Prometheus

`deploy-remote.sh`'s output already includes a ready-to-paste snippet. In
general:

```yaml
scrape_configs:
  - job_name: zyvor-device-agent
    static_configs:
      - targets: ['host:9188']
    authorization:      # only needed when auth.mode = "bearer"
      type: Bearer
      credentials: <TOKEN>
```

If the agent is bound to loopback only (the default), run the scraper (or
a Prometheus agent/relay) on the same host and target `127.0.0.1:9188` —
an external Prometheus can't reach a loopback-only bind.

## Next steps

- [`ARCHITECTURE.md`](../ARCHITECTURE.md) — the full security-model
  picture and what's still open before GA.
- [`ROADMAP.md`](../ROADMAP.md) / [`BACKLOG.md`](../BACKLOG.md) — arm64
  packaging and everything else planned past this.
- You've now been through the full tutorial series — the README's
  [Documentation](https://github.com/zyvorai/device-agent#documentation) section is your map back
  to any reference doc.
