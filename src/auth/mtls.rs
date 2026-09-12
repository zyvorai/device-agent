// SPDX-License-Identifier: Apache-2.0

//! `auth.mode = "mtls"`: the TCP listener terminates TLS itself using the
//! identity written by `zyvor-device-agent enroll` (see [`super::enroll`]),
//! served via `axum-server`'s rustls integration rather than hand-rolling a
//! rustls+hyper accept loop. When `require_client_cert` is set, an incoming
//! connection that doesn't present a certificate signed by `client_ca_file`
//! is rejected at the TLS handshake, before any application code runs.
//!
//! The device's own certificate is always loaded as plain PEM (issued certs
//! aren't secret). Its *private key* may instead be a TPM-backed
//! [`crate::identity::DeviceIdentity::Tpm`] key (`identity.backend = "tpm"`,
//! `--features tpm2`) - in which case every TLS handshake signature is
//! performed by the TPM itself via a custom `rustls::sign::SigningKey`,
//! rather than loading a private key file at all.

use std::{fs, io::BufReader, sync::Arc};

use anyhow::{bail, Context};
use axum_server::tls_rustls::RustlsConfig;
#[cfg(feature = "tpm2")]
use rustls::sign::{CertifiedKey, SingleCertAndKey};
use rustls::{server::WebPkiClientVerifier, RootCertStore, ServerConfig as RustlsServerConfig};

use crate::{config::Config, identity::DeviceIdentity};

/// Loads the identity/trust material named by `cfg` and builds the rustls
/// server config `axum-server` will terminate TLS with.
pub async fn load_server_config(cfg: &Config) -> anyhow::Result<RustlsConfig> {
    let mtls = &cfg.auth.mtls;
    let cert_chain = load_certs(&mtls.cert_file)?;
    let identity = DeviceIdentity::load(cfg, &mtls.key_file)
        .with_context(|| format!("loading device identity from {}", mtls.key_file))?;
    tracing::info!(backend = identity.backend_name(), "mTLS identity loaded");

    let builder = RustlsServerConfig::builder();
    let with_client_verifier = if mtls.require_client_cert {
        if mtls.client_ca_file.is_empty() {
            bail!("auth.mtls.require_client_cert = true but client_ca_file is empty");
        }
        let mut roots = RootCertStore::empty();
        for cert in load_certs(&mtls.client_ca_file)? {
            roots
                .add(cert)
                .context("adding client_ca_file entry to trust store")?;
        }
        let client_verifier = WebPkiClientVerifier::builder(Arc::new(roots))
            .build()
            .context("building client certificate verifier")?;
        builder.with_client_cert_verifier(client_verifier)
    } else {
        // TLS without the "m": still this device's own cert on the wire,
        // but no client certificate is required to connect.
        builder.with_no_client_auth()
    };

    let mut server_config = match identity {
        DeviceIdentity::Software(ref software) => {
            let key = rustls_pemfile::private_key(&mut BufReader::new(
                software.serialize_pem().as_bytes(),
            ))
            .context("parsing device private key")?
            .context("no private key material for the software identity")?;
            with_client_verifier
                .with_single_cert(cert_chain, key)
                .context("building TLS server config from device identity")?
        }
        #[cfg(feature = "tpm2")]
        DeviceIdentity::Tpm(_) => {
            let signing_key = identity
                .rustls_signing_key()
                .context("building rustls signing key from the TPM identity")?;
            let certified_key = CertifiedKey::new(cert_chain, signing_key);
            with_client_verifier.with_cert_resolver(Arc::new(SingleCertAndKey::from(certified_key)))
        }
    };
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(RustlsConfig::from_config(Arc::new(server_config)))
}

fn load_certs(path: &str) -> anyhow::Result<Vec<rustls::pki_types::CertificateDer<'static>>> {
    let file = fs::File::open(path).with_context(|| format!("opening {path}"))?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("parsing PEM certificates from {path}"))
}
