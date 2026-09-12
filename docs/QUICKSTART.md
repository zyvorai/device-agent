---
hero:
  eyebrow: QUICKSTART
  title: 'Quickstart: packaged install with bearer auth'
---

This walks through the systemd deployment path end to end: install, enable
bearer auth, connect the dashboard, and scrape Prometheus metrics. For a
from-source `cargo run` loop instead, see the README's own "Quick start".

## 1. Deploy

From a workstation with SSH access to the target Linux host:

```bash
./scripts/deploy-remote.sh user@host --auth-mode bearer
```

This builds the agent and dashboard on the remote host, installs the
systemd unit, and — because of `--auth-mode bearer` — generates a bearer
token, hashes it (the daemon never stores the raw token), and prints:

```
==> Bearer token (generated · save this, it is not stored on the remote): <TOKEN>
==> Retrieve the hash later with: ssh user@host 'sudo -n cat /etc/zyvor/device-agent/auth/bearer.sha256'
==> /metrics requires the same bearer token. Prometheus scrape-config snippet:
...
```

**Save that token now** — it is shown exactly once. If you lose it, redeploy
with `FORCE_BEARER_TOKEN_RESET=1 ./scripts/deploy-remote.sh user@host --auth-mode bearer`
to issue a new one.

Without `--auth-mode bearer`, the agent deploys with `auth.mode = "none"` —
fine for `--bind 127.0.0.1` (the default), but `deploy-remote.sh` refuses to
bind a non-loopback address with no auth configured.

## 2. Confirm it's running

```bash
ZYVOR_DEVICE_AGENT_BEARER_TOKEN=<TOKEN> ./scripts/verify-deployment.sh host user
```

Or by hand:

```bash
curl -sS http://127.0.0.1:9188/api/v1/health          # always unauthenticated
curl -sS http://127.0.0.1:9188/api/v1/inventory        # 401 without a token
curl -sS -H "Authorization: Bearer <TOKEN>" \
  http://127.0.0.1:9188/api/v1/inventory               # 200 with the right one
```

## 3. Connect the dashboard

Open `http://host:9188/` (or `http://127.0.0.1:9188/` if you're on the host,
or via an SSH tunnel if the agent is bound to loopback only, which is the
default). The dashboard shell itself always loads without a token — only the
API calls it makes require one. On first load against a bearer-protected
agent, it shows an "authentication required" banner: paste the token there
(or later, in Settings → Authentication). It's stored in that browser's
`localStorage` only, never sent anywhere but this agent.

## 4. Scrape metrics with Prometheus

`deploy-remote.sh`'s output already includes a ready-to-paste snippet. The
general shape (see `docs/observability/prometheus-scrape-example.yml` for a
full file, and `docs/observability/grafana-dashboard.json` for a starter
dashboard covering it):

```yaml
scrape_configs:
  - job_name: zyvor-device-agent
    static_configs:
      - targets: ['host:9188']
    authorization:
      type: Bearer
      credentials: <TOKEN>
```

If the agent is bound to loopback only (the default), run the scraper (or a
Prometheus agent/relay) on the same host and target `127.0.0.1:9188`
instead — an external Prometheus can't reach a loopback-only bind.

## Next steps

- `docs/HARDWARE_PERMISSIONS.md` — bus permissions for dropped-privilege
  sensor plugins (`plugins.run_as_uid`/`run_as_gid`), plus the opt-in
  seccomp-bpf denylist (`plugins.seccomp_enabled`).
- `docs/MTLS_ENROLLMENT.md` — `auth.mode = "mtls"` as an alternative to
  bearer auth: `zyvor-device-agent enroll`/`identity`, and (optionally)
  `docs/TPM2_IDENTITY.md` for keeping that private key inside a TPM2.
- README's "CORS and rate limiting", "Configurable health thresholds" and
  "Config hot-reload" sections for the other opt-in
  `[server.*]`/`[thresholds]`/`SIGHUP` behavior.
- README's "Packaging" and "Hotplug" sections if you're installing from a
  `.deb`/`.rpm` or want faster-than-polling bus-change detection.
- `docs/ARCHITECTURE.md` for the full security-model picture and what's
  still open before GA.
