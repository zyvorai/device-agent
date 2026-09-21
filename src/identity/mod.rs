// SPDX-License-Identifier: Apache-2.0

//! Abstraction over *where the mTLS device private key lives and signs*,
//! used by [`crate::auth::enroll`] (generating/persisting the key) and
//! [`crate::auth::mtls`] (serving with it). Two backends:
//!
//! - [`software`] (always compiled): today's plain PKCS#8-file-on-disk key.
//! - [`tpm`] (`#[cfg(feature = "tpm2")]`, off by default): the private key
//!   is generated inside a TPM2 and never leaves it in plaintext form - the
//!   on-disk artifact is a TPM-wrapped key blob, not a raw PKCS#8 key. See
//!   `docs/TPM2_IDENTITY.md`.
//!
//! `identity.policy` controls failure:
//! - `software` — always a file key
//! - `preferred` (default) — TPM when `backend = "tpm"`, else software, with
//!   fallback if the TPM cannot be opened
//! - `required` — fail if the TPM cannot be opened or this binary lacks `tpm2`

pub mod software;
#[cfg(feature = "tpm2")]
pub mod tpm;

use std::{fs, path::Path, sync::Arc};

use anyhow::Context;

use crate::config::Config;

/// The device's mTLS private key, backed by whichever provider actually
/// created/loaded it.
pub enum DeviceIdentity {
    // Boxed: an rcgen::KeyPair is much larger than the Arc-based tpm2
    // variant, and clippy (correctly) flags the resulting size skew.
    Software(Box<software::SoftwareKey>),
    #[cfg(feature = "tpm2")]
    Tpm(tpm::TpmKey),
}

impl DeviceIdentity {
    /// Generates a fresh key using `identity.policy` and `identity.backend`.
    pub fn generate(cfg: &Config) -> anyhow::Result<Self> {
        match cfg.identity.policy.as_str() {
            "software" => {
                if cfg.identity.backend == "tpm" {
                    tracing::warn!(
                        "identity.policy = \"software\" overrides identity.backend = \"tpm\""
                    );
                }
                return Ok(Self::Software(Box::new(software::SoftwareKey::generate()?)));
            }
            "preferred" | "required" => {}
            other => anyhow::bail!(
                "identity.policy must be software, preferred, or required (got {other})"
            ),
        }

        let require_tpm = cfg.identity.policy == "required";
        let try_tpm = require_tpm || cfg.identity.backend == "tpm";
        if !try_tpm {
            return Ok(Self::Software(Box::new(software::SoftwareKey::generate()?)));
        }

        #[cfg(feature = "tpm2")]
        {
            match tpm::TpmKey::generate(&cfg.identity.tpm_tcti) {
                Ok(key) => return Ok(Self::Tpm(key)),
                Err(error) if require_tpm => {
                    anyhow::bail!(
                        "identity.policy = \"required\" but the TPM could not be opened: {error}"
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        %error,
                        "identity.policy = \"preferred\" and the TPM could not be opened; \
                         falling back to a software key"
                    );
                }
            }
        }
        #[cfg(not(feature = "tpm2"))]
        {
            if require_tpm {
                anyhow::bail!(
                    "identity.policy = \"required\" but this binary was built without the tpm2 feature"
                );
            }
            tracing::warn!(
                "identity.backend = \"tpm\" but this binary was built without the tpm2 \
                 feature; falling back to a software key"
            );
        }
        Ok(Self::Software(Box::new(software::SoftwareKey::generate()?)))
    }

    /// Fail closed before `serve` when production policy demands a TPM.
    pub fn assert_serve_policy(cfg: &Config) -> anyhow::Result<()> {
        if cfg.identity.policy != "required" {
            return Ok(());
        }
        #[cfg(not(feature = "tpm2"))]
        {
            anyhow::bail!(
                "identity.policy = \"required\" but this binary was built without the tpm2 feature"
            );
        }
        #[cfg(feature = "tpm2")]
        {
            tpm::probe(&cfg.identity.tpm_tcti)?;
            if std::path::Path::new(&cfg.auth.mtls.key_file).exists() {
                let contents = fs::read_to_string(&cfg.auth.mtls.key_file)
                    .with_context(|| format!("reading {}", cfg.auth.mtls.key_file))?;
                if contents.trim_start().starts_with("-----BEGIN") {
                    anyhow::bail!(
                        "identity.policy = \"required\" but {} is a software private key",
                        cfg.auth.mtls.key_file
                    );
                }
            }
            Ok(())
        }
    }

    /// Reloads whatever backend previously wrote `key_file`, detected from
    /// the file's own content: a TPM key file is a small JSON envelope, a
    /// software key file is a PEM-encoded PKCS#8 key.
    pub fn load(cfg: &Config, key_file: &str) -> anyhow::Result<Self> {
        let contents =
            fs::read_to_string(key_file).with_context(|| format!("reading {key_file}"))?;
        if contents.trim_start().starts_with("-----BEGIN") {
            return Ok(Self::Software(Box::new(software::SoftwareKey::from_pem(
                &contents,
            )?)));
        }
        #[cfg(feature = "tpm2")]
        {
            Ok(Self::Tpm(tpm::TpmKey::load(
                &contents,
                &cfg.identity.tpm_tcti,
            )?))
        }
        #[cfg(not(feature = "tpm2"))]
        {
            let _ = cfg;
            anyhow::bail!(
                "{key_file} looks like a TPM key blob but this binary was built without the \
                 tpm2 feature"
            )
        }
    }

    pub fn persist(&self, key_file: &str) -> anyhow::Result<()> {
        match self {
            Self::Software(key) => write_identity_file(key_file, key.serialize_pem().as_bytes()),
            #[cfg(feature = "tpm2")]
            Self::Tpm(key) => write_identity_file(key_file, key.serialize_json()?.as_bytes()),
        }
    }

    pub fn backend_name(&self) -> &'static str {
        match self {
            Self::Software(_) => "software",
            #[cfg(feature = "tpm2")]
            Self::Tpm(_) => "tpm",
        }
    }

    /// A CSR-generation-capable signing key (see [`rcgen::SigningKey`]).
    ///
    /// Returns a small `Sized` enum rather than `&dyn rcgen::SigningKey`:
    /// `rcgen::CertificateParams::serialize_request` takes `&impl SigningKey`,
    /// which (like any bare generic parameter) implicitly requires `Sized` -
    /// a trait object can't satisfy that.
    pub fn rcgen_signing_key(&self) -> RcgenSigningKeyRef<'_> {
        match self {
            Self::Software(key) => RcgenSigningKeyRef::Software(&key.key_pair),
            #[cfg(feature = "tpm2")]
            Self::Tpm(key) => RcgenSigningKeyRef::Tpm(key),
        }
    }

    /// A TLS-serving-capable signing key for [`crate::auth::mtls`].
    pub fn rustls_signing_key(&self) -> anyhow::Result<Arc<dyn rustls::sign::SigningKey>> {
        match self {
            Self::Software(_) => {
                anyhow::bail!("rustls_signing_key is only used by the TPM backend today")
            }
            #[cfg(feature = "tpm2")]
            Self::Tpm(key) => Ok(Arc::new(key.clone())),
        }
    }
}

/// A `Sized` handle to whichever backend's signing key is active, so it can
/// be passed to APIs generic over `impl rcgen::SigningKey` (see
/// [`DeviceIdentity::rcgen_signing_key`]).
pub enum RcgenSigningKeyRef<'a> {
    Software(&'a rcgen::KeyPair),
    #[cfg(feature = "tpm2")]
    Tpm(&'a tpm::TpmKey),
}

impl rcgen::PublicKeyData for RcgenSigningKeyRef<'_> {
    fn der_bytes(&self) -> &[u8] {
        match self {
            Self::Software(key) => key.der_bytes(),
            #[cfg(feature = "tpm2")]
            Self::Tpm(key) => key.der_bytes(),
        }
    }

    fn algorithm(&self) -> &'static rcgen::SignatureAlgorithm {
        match self {
            Self::Software(key) => key.algorithm(),
            #[cfg(feature = "tpm2")]
            Self::Tpm(key) => key.algorithm(),
        }
    }
}

impl rcgen::SigningKey for RcgenSigningKeyRef<'_> {
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, rcgen::Error> {
        match self {
            Self::Software(key) => key.sign(msg),
            #[cfg(feature = "tpm2")]
            Self::Tpm(key) => key.sign(msg),
        }
    }
}

pub(crate) fn write_identity_file(path: &str, contents: &[u8]) -> anyhow::Result<()> {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("writing {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Private keys and TPM key blobs alike: readable only by the owner
        // (expected to be the daemon's own uid, today root).
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("setting permissions on {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn software_policy_ignores_tpm_backend() {
        let mut cfg = Config::default();
        cfg.identity.policy = "software".into();
        cfg.identity.backend = "tpm".into();
        let identity = DeviceIdentity::generate(&cfg).unwrap();
        assert_eq!(identity.backend_name(), "software");
    }

    #[test]
    fn required_policy_fails_closed_without_a_tpm() {
        let mut cfg = Config::default();
        cfg.identity.policy = "required".into();
        let generated = DeviceIdentity::generate(&cfg);
        assert!(generated.is_err(), "required policy must not return a key");
        let serve = DeviceIdentity::assert_serve_policy(&cfg);
        assert!(serve.is_err());
    }
}
