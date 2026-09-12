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
//! `identity.backend = "tpm"` falls back to software at runtime (with a
//! warning) if the TPM can't be opened, since most dev/test boxes have none.

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
    /// Generates a fresh key using the backend named by `cfg.identity.backend`.
    pub fn generate(cfg: &Config) -> anyhow::Result<Self> {
        match cfg.identity.backend.as_str() {
            "tpm" => {
                #[cfg(feature = "tpm2")]
                {
                    match tpm::TpmKey::generate(&cfg.identity.tpm_tcti) {
                        Ok(key) => return Ok(Self::Tpm(key)),
                        Err(error) => {
                            tracing::warn!(
                                %error,
                                "identity.backend = \"tpm\" but the TPM could not be opened; \
                                 falling back to a software key"
                            );
                        }
                    }
                }
                #[cfg(not(feature = "tpm2"))]
                tracing::warn!(
                    "identity.backend = \"tpm\" but this binary was built without the tpm2 \
                     feature; falling back to a software key"
                );
                Ok(Self::Software(Box::new(software::SoftwareKey::generate()?)))
            }
            _ => Ok(Self::Software(Box::new(software::SoftwareKey::generate()?))),
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
