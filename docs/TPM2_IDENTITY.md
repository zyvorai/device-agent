# TPM2-backed mTLS identity (v0.1.4, optional)

`identity.backend = "tpm"` moves the mTLS private key introduced in
`docs/MTLS_ENROLLMENT.md` into a TPM2, so it is generated and signs entirely
inside the chip instead of living as a PKCS#8 file on disk. Fully optional
and feature-gated (`--features tpm2`, off by default) - it must never affect
a plain `cargo build`, and most dev/test machines have no TPM at all.

## What actually changes

Everything else about enrollment and mTLS serving stays the same (same
`enroll`/`identity` commands, same wire protocol, same `auth.mode = "mtls"`
listener). Only *where the private key lives and signs* changes:

- **software** (default): `rcgen`/`ring` generate an ECDSA P-256 key; it's
  written to `auth.mtls.key_file` as a plain PKCS#8 PEM file.
- **tpm**: a `tss-esapi` `TransientKeyContext` asks the TPM to generate an
  ECDSA P-256 key. What's written to `auth.mtls.key_file` instead is a small
  JSON envelope holding the TPM's public key plus an *encrypted* private key
  blob that only the same TPM can unwrap - never a raw PKCS#8 key. Every
  signature (the CSR at enrollment time, every TLS handshake while serving)
  re-opens a TPM session and asks the chip to sign; the private key's
  sensitive area is decrypted by the TPM itself and never exists in usable
  form in this process's memory.

`zyvor-device-agent identity` reports which backend produced the current
key (`backend: software` or `backend: tpm`).

## Runtime fallback

`identity.backend = "tpm"` is a *request*, not a guarantee: if the TPM can't
be opened (no TPM present, wrong TCTI, permission denied), `enroll` and
`serve` both log a warning and fall back to a software key rather than
failing outright - most dev/test boxes have no TPM, and a fleet with mixed
hardware shouldn't need a different config per device just for this.

## Configuration

```toml
[identity]
backend = "tpm"
tpm_tcti = "device:/dev/tpmrm0"   # or e.g. "swtpm:host=127.0.0.1,port=2321"
```

`tpm_tcti` is a `tss-esapi` TCTI connection string. Empty uses
`tss-esapi`'s own environment-variable-based default resolution
(`TPM2TOOLS_TCTI`/`TCTI`). A real TPM is typically `device:/dev/tpmrm0`
(the kernel resource-managed TPM device node, preferred over `/dev/tpm0`
so multiple processes can share the chip safely).

## Why ECC P-256 only

The `tpm2` feature always uses ECDSA P-256/SHA-256 - the combination every
TPM2 chip and `swtpm` support well - rather than exposing a wider algorithm
choice. This keeps the DER signature encoding and public-key handling in
`src/identity/tpm.rs` simple and narrows what needs security review.

## Live-verified with `swtpm`

This was verified end to end against `swtpm` (the TPM2 emulator used for
development and CI where no physical TPM is available), not just
compile-checked:

```bash
swtpm socket --tpm2 --tpmstate dir=/var/lib/swtpm --ctrl type=tcp,port=2322 \
  --server type=tcp,port=2321 &
swtpm_ioctl --tcp :2322 -i                        # one-time hardware init
tpm2_startup --clear --tcti=swtpm:host=127.0.0.1,port=2321

# identity.backend = "tpm", identity.tpm_tcti = "swtpm:host=127.0.0.1,port=2321"
zyvor-device-agent --config device-agent.toml enroll
zyvor-device-agent --config device-agent.toml identity   # backend: tpm
zyvor-device-agent --config device-agent.toml serve       # auth.mode = "mtls"
```

Confirmed: the persisted `auth.mtls.key_file` is the JSON envelope described
above (not a PKCS#8 key), `identity` reports `backend: tpm`, and a real mTLS
handshake against the running `serve` process succeeds - i.e. the TLS
signature the TPM produced during the handshake verifies correctly against
the certificate's public key.

## Non-goals

No support for moving an *existing* software key into a TPM, or vice versa -
`enroll --force` against the desired backend issues a fresh key either way.
Multiple simultaneous identities (e.g. software while migrating to TPM) are
not supported; `auth.mtls.key_file` holds exactly one.
