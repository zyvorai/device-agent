# 3. Securing your agent

Device Agent supports three auth modes plus an optional same-host Unix
socket. This guide is a decision tree; each mode's full setup lives in its
own reference doc, linked below.

## Which mode do I need?

| Situation | Use |
|---|---|
| Local development, bound to `127.0.0.1` only | `auth.mode = "none"` (the default) |
| Bound to a non-loopback address, single shared secret is fine | `auth.mode = "bearer"` |
| Bound to a non-loopback address, want per-device certificate identity (fleet of many devices, cert rotation, no shared secret to leak) | `auth.mode = "mtls"` |
| Fleet/Nodra running on the same host and would rather use kernel peer-credential checks than carry a token | `[server.unix_socket]`, additive to whichever of the above you pick for the TCP listener |

`auth.mode = "none"` on anything but a loopback bind is refused by
`scripts/deploy-remote.sh` for exactly this reason — don't do it by hand
either.

## `none` (default)

No setup. Every `/api/*` route and `/metrics` is open to anyone who can
reach the port. Fine for `--bind 127.0.0.1` during development; never for a
production, non-loopback bind.

## `bearer`

```toml
[auth]
mode = "bearer"
exempt_paths = ["/api/v1/health", "/api/v1/ready"]

[auth.bearer]
token_hash_file = "/etc/zyvor/device-agent/auth/bearer.sha256"
```

The daemon never stores the raw token, only its SHA-256 hash. Generate one
and populate the hash file automatically with:

```bash
./scripts/deploy-remote.sh HOST --auth-mode bearer
```

which prints the token exactly once — save it immediately. Every
authenticated request needs `Authorization: Bearer <token>`; `/metrics`
needs it too (it is **not** in `exempt_paths` by default). The bundled
dashboard prompts for the token on first load and stores it in that
browser's `localStorage` only. See the README's "API auth and Unix socket"
section for the full picture, including the two SSE routes that accept the
token as a `?token=` query parameter (browsers' `EventSource` can't set
headers).

## `mtls`

```toml
[auth]
mode = "mtls"

[auth.mtls]
require_client_cert = true
client_ca_file = "/etc/zyvor/device-agent/identity/client-ca.pem"
```

Auth happens at the TLS handshake, before any request reaches application
code. Before `serve` will start in this mode, run enrollment once:

```bash
cargo run -- --config /etc/zyvor/device-agent.toml enroll
cargo run -- --config /etc/zyvor/device-agent.toml identity   # confirm subject/issuer/validity
```

`enroll` generates a keypair and CSR and submits them to
`enrollment.server_url`; Device Agent never signs certificates itself. Full
protocol and rationale: [`MTLS_ENROLLMENT.md`](../MTLS_ENROLLMENT.md).

If you want that private key generated and signed inside a TPM2 instead of
a plain PKCS#8 file, set `identity.backend = "tpm"` (build with `--features
tpm2`) — falls back to software at runtime with a warning if the TPM can't
be opened. See [`TPM2_IDENTITY.md`](../TPM2_IDENTITY.md).

`scripts/deploy-remote.sh --auth-mode` only knows `none`/`bearer` today —
set up mTLS by hand with `enroll` as shown above, then deploy.

## Same-host Unix socket (additive)

```toml
[server.unix_socket]
enabled = true
path = "/run/zyvor-device-agent/api.sock"
allow_uids = [1000]
allow_gids = []
```

A second API listener, kernel peer-credential checked instead of
token/cert checked. Empty `allow_uids`/`allow_gids` deny everyone — both
must be explicitly populated. Useful for Fleet/Nodra running as a
well-known local uid that would rather not carry a bearer token or client
cert.

## CORS and rate limiting

Both apply to the TCP listener only (the Unix socket has no concept of
either):

```toml
[server.cors]
enabled = false          # off by default — the bundled dashboard is same-origin
allowed_origins = []

[server.rate_limit]
enabled = true           # on by default, generous limits — pure DoS protection
requests_per_second = 20
burst = 40
```

Only turn CORS on if a dashboard or integration is served from a different
origin than the agent itself.

## Next steps

- [4. Industrial buses](04-industrial-buses.md) — the next section of
  `device-agent.toml` you'll likely touch.
- [6. Deploying to production](06-deploying-to-production.md) — how
  `deploy-remote.sh` and the `.deb`/`.rpm` packages wire these settings up
  end to end.
