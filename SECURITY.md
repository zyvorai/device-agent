# Security Policy

Do not open public issues for suspected security vulnerabilities. Report security issues privately to the Zyvor security contact published at https://zyvor.dev.

## v0.1 security boundaries

- The daemon is intended to run on a trusted Linux edge node.
- Physical bus access is local-only through the agent and explicit plugins.
- Plugin manifests must be provisioned by an administrator; arbitrary remote command execution is not a feature.
  As of v0.1.4, plugin subprocesses can additionally be run under a
  dropped-privilege uid/gid (`plugins.run_as_uid`/`run_as_gid`) and behind an
  opt-in seccomp-bpf denylist (`plugins.seccomp_enabled`) — see
  `docs/PLUGIN_PROTOCOL.md` and `docs/HARDWARE_PERMISSIONS.md`.
- Fleet tokens and Nodra credentials should be supplied through protected configuration or environment injection.
- The REST listener should be firewalled or bound to localhost until authentication is enabled for a deployment.
  As of v0.1.4, `auth.mode` can be `"bearer"` (token) or `"mtls"` (client
  certificate, verified at the TLS handshake) — see `docs/MTLS_ENROLLMENT.md`.
  The mTLS private key can optionally be generated and held inside a TPM2
  (`identity.backend = "tpm"`, `--features tpm2`) rather than as a plaintext
  file — see `docs/TPM2_IDENTITY.md`.
- Device Agent does not operate a certificate authority: `zyvor-device-agent
  enroll` is a client submitting a CSR to an operator-run enrollment
  endpoint, never a signer. Building and operating that endpoint (key
  custody, revocation, rotation policy) is the deploying organization's
  responsibility.
