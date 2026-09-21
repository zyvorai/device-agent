---
hero:
  eyebrow: UPGRADE
  title: Upgrading from v0.1.6 to v0.2.0
---

## Breaking changes

1. **Listen default is loopback.** Packaged config and `Config` defaults use
   `127.0.0.1:9188`. A non-loopback bind with `auth.mode = "none"` refuses to
   start unless `server.allow_unauthenticated_remote = true`.
2. **Query-string bearer tokens are gone.** `?token=` no longer authenticates
   SSE or camera routes. Issue a short-lived ticket with
   `POST /api/v1/stream-tickets`, or use `fetch()` with `Authorization`.
3. **TPM policy.** `identity.policy = "required"` fails closed when the TPM is
   missing. The default remains `preferred` for CI and laptops.

## New local surfaces

- Device passport: `GET /api/v1/passport`, `agentctl passport`
- Flight recorder: `GET /api/v1/recorder?since=15m`
- Support bundles: `agentctl support-bundle --since 2h --redact`
- Safe remediation: `POST /api/v1/remediation` (signed allowlist, no shell)
- Commissioning: `agentctl commission` and `scripts/install.sh --enrollment-token`

## Minew

Physical Minewing HIL remains unsigned. Emulator and lab-surrogate runs must
keep `minewing_claimable=false`. Do not market Minew support as certified.
