# mTLS enrollment (v0.1.4)

`auth.mode = "mtls"` moves API authentication to the TLS layer itself: the
daemon terminates TLS using its own issued certificate and (when
`require_client_cert = true`) rejects any connection that doesn't present a
client certificate signed by a trusted CA - before the request ever reaches
application code.

## Scope: client side only

Device Agent implements only its side of this handshake: generating a
keypair and CSR, submitting them to a configured enrollment endpoint, and
persisting whatever certificate comes back. It does not implement a
certificate authority. Today, `zyvorai/fleet`'s enrollment-token system
issues an opaque bearer-style token, not a signed certificate - there is no
CSR-signing endpoint anywhere in this stack yet. Building a production CA
(key custody, revocation, rotation policy) is a separate, security-sensitive
project of its own and deliberately isn't improvised here.

## Enrollment protocol

`zyvor-device-agent enroll` does the following:

1. Reads a single-use token from `enrollment.token_file`.
2. Generates a keypair and a CSR (`Common Name` = `enrollment.common_name`,
   or `device.serial` if that's empty).
3. `POST`s to `enrollment.server_url` with `Authorization: Bearer <token>`
   and a JSON body `{"csr_pem": "...", "common_name": "..."}`.
4. Expects a 200 response `{"certificate_pem": "...", "ca_bundle_pem": "..."}`.
5. Writes the certificate to `auth.mtls.cert_file`, the private key to
   `auth.mtls.key_file` (mode `0600`, never transmitted), and - if returned
   and `auth.mtls.client_ca_file` is set - the CA bundle to that path.
6. Deletes the (now-consumed) token file.

Re-running `enroll` refuses to overwrite an existing certificate unless
`--force` is passed, so a stray re-run can't silently replace a working
identity.

## Serving `auth.mode = "mtls"`

`serve` refuses to start in `mtls` mode if `auth.mtls.cert_file`/`key_file`
don't exist yet - run `enroll` first. When they're present, the TCP listener
is served via `axum-server`'s rustls integration instead of plain `axum::serve`,
with the device's own certificate as the server identity. Set
`auth.mtls.require_client_cert = true` and point `client_ca_file` at the CA
that signs client certificates to require mutual TLS; leave it `false` for
server-side TLS only (the daemon still proves its own identity, but accepts
any client).

`zyvor-device-agent identity` prints the current certificate's subject,
issuer, and validity window (`backend: software` today; a TPM-backed key
provider is a future, feature-gated addition - see `docs/ROADMAP.md`).

## Verifying locally with a throwaway test CA

Nothing in this repository stands up a CA - for local verification, run any
minimal HTTPS server that accepts the POST above and signs the CSR with a
throwaway root (e.g. `openssl ca` or a few lines of `cryptography`/`rcgen`).
Never point `enrollment.server_url` at a throwaway CA in a real deployment;
it exists purely to exercise this client-side flow end to end.

```bash
sudo zyvor-device-agent --config /etc/zyvor/device-agent.toml enroll
sudo zyvor-device-agent --config /etc/zyvor/device-agent.toml identity

curl --cacert client-ca.pem --cert client.pem --key client-key.pem \
  https://device:9188/api/v1/health   # succeeds with a valid client cert
curl --cacert client-ca.pem https://device:9188/api/v1/health
  # rejected at the TLS handshake with require_client_cert = true
```

## Non-goals

Automatic certificate rotation and CRL/OCSP revocation checking are
explicitly out of scope for this milestone - reissue manually with
`enroll --force` when a certificate needs replacing.
