# Troubleshooting

Real issues, with the actual fix — not a generic checklist. If your symptom
isn't here, check `journalctl -u zyvor-device-agent -f` first (or
`docker logs`/`podman logs` for the container path), then
[open an issue](https://github.com/zyvorai/device-agent/issues).

## Deploy/verify scripts report `HEALTH_FAIL` / `INVENTORY_FAIL` / `METRICS_FAIL`

**This is expected, not a broken deployment, once `server.tls.enabled =
true`.** `scripts/deploy-remote.sh`'s and `scripts/verify-deployment.sh`'s
built-in checks curl plain `http://127.0.0.1:PORT` — against a daemon now
correctly speaking TLS-only, that fails closed exactly as it should.
Service-active plus the deploy step itself succeeding already confirm the
deployment worked. Verify by hand instead:

```bash
curl -sSk https://127.0.0.1:9188/api/v1/health
```

(`-k` accepts the auto-generated self-signed cert; drop it if you've
mounted a real one.) See the README's [TLS](../README.md#tls-v015-optional)
section.

## Browser shows "Your connection is not private" / a certificate warning

Expected the first time you reach a `server.tls.enabled = true` listener —
the daemon auto-generates a self-signed certificate if none is configured
(see [`src/tls.rs`](../src/tls.rs)). Either click through the browser's
warning once, or replace it with a real certificate by mounting it at
`server.tls.cert_path`/`key_path` (an existing cert/key there is never
overwritten).

## Lost or forgotten the bearer token

The daemon never stores the raw token — only its SHA-256 hash
(`auth.bearer.token_hash_file`), by design. There is no way to recover a
lost token; reissue a new one:

```bash
FORCE_BEARER_TOKEN_RESET=1 ./scripts/deploy-remote.sh user@host --auth-mode bearer
```

This prints the new token once — save it immediately. See
[3. Securing your agent](guides/03-securing-your-agent.md).

## Camera snapshot/stream returns 503, or the capture log shows "Permission denied"

`/dev/video*` needs either root (the daemon's default) or membership in the
`video` group if you've de-privileged the daemon — see
[`docs/HARDWARE_PERMISSIONS.md`](HARDWARE_PERMISSIONS.md)'s camera row. A
503 with no permission error in the log usually just means the capture
thread hasn't produced a frame yet — check `GET /api/v1/camera` for
`capturing`/`last_error` before assuming a permissions problem.

## Dashboard is stuck showing "Authentication required"

Confirm `auth.mode` on the agent and that you've entered the correct
token in the dashboard's prompt (Settings → Authentication) — it's stored
per-browser in `localStorage`, so a new browser/incognito window needs it
re-entered. Note the dashboard's static shell always loads without a
token, even in `auth.mode = "bearer"`; only its API calls need one, so a
loading dashboard with a persistent auth banner is normal, not a sign
something else is broken.

## `deploy-remote.sh` refuses to deploy with `--bind` set to a non-loopback address

This is intentional: `auth.mode = "none"` (the default when seeding a new
config) is refused on anything but a loopback bind, to stop an
unauthenticated agent from accidentally landing on a reachable network.
Pass `--auth-mode bearer`, or configure `server.tls.enabled = true` and set
`auth.mode` yourself before binding non-loopback.

## Sent `SIGHUP` but a config change didn't take effect

Not every field is hot-reloadable. `server.listen`, `server.unix_socket.*`,
and `server.dashboard_dir` are bound once at startup; the Nodra MQTT
connection itself and which interfaces `industrial.can_capture` has open
are also only read once. A reload applies everything else and logs a
warning naming which of these it couldn't apply — restart the service for
those. See the README's
[Config hot-reload](../README.md#config-hot-reload-v014) section.

## `doctor` (or the dashboard's Diagnostics page) reports failing checks

Expected in a container, CI runner, or dev VM with no real GPIO/I2C/CAN
hardware attached — `doctor` reports what it actually finds, and an empty
bus is a real (if uninteresting) result there, not a bug. This is exactly
why `ci.yml`'s container smoke test treats a non-zero `doctor` exit as
expected rather than a failure.

## Nothing in this list matches

Check the relevant reference doc first — `docs/CAMERA.md`,
`docs/CAN_CAPTURE.md`, `docs/MTLS_ENROLLMENT.md`, `docs/PLUGIN_PROTOCOL.md`
each have their own safety-contract and gotcha sections — then
[open an issue](https://github.com/zyvorai/device-agent/issues) with your config (redact secrets) and the
relevant `journalctl`/log output.
