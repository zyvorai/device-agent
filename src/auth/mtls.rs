// SPDX-License-Identifier: Apache-2.0

//! `auth.mode = "mtls"`: the TCP listener terminates TLS itself using the
//! identity written by `zyvor-device-agent enroll` (see [`super::enroll`]),
//! served via `axum-server`'s rustls integration rather than hand-rolling a
//! rustls+hyper accept loop. When `require_client_cert` is set, an incoming
//! connection that doesn't present a certificate signed by `client_ca_file`
//! is rejected at the TLS handshake, before any application code runs.

use std::{fs, io::BufReader, sync::Arc};

use anyhow::{bail, Context};
use axum_server::tls_rustls::RustlsConfig;
use rustls::{server::WebPkiClientVerifier, RootCertStore, ServerConfig as RustlsServerConfig};

use crate::config::MtlsAuthConfig;

/// Loads the identity/trust material named by `cfg` and builds the rustls
/// server config `axum-server` will terminate TLS with.
pub async fn load_server_config(cfg: &MtlsAuthConfig) -> anyhow::Result<RustlsConfig> {
    let cert_chain = load_certs(&cfg.cert_file)?;
    let key = load_key(&cfg.key_file)?;

    let builder = RustlsServerConfig::builder();
    let mut server_config = if cfg.require_client_cert {
        if cfg.client_ca_file.is_empty() {
            bail!("auth.mtls.require_client_cert = true but client_ca_file is empty");
        }
        let mut roots = RootCertStore::empty();
        for cert in load_certs(&cfg.client_ca_file)? {
            roots
                .add(cert)
                .context("adding client_ca_file entry to trust store")?;
        }
        let client_verifier = WebPkiClientVerifier::builder(Arc::new(roots))
            .build()
            .context("building client certificate verifier")?;
        builder
            .with_client_cert_verifier(client_verifier)
            .with_single_cert(cert_chain, key)
    } else {
        // TLS without the "m": still this device's own cert on the wire,
        // but no client certificate is required to connect.
        builder
            .with_no_client_auth()
            .with_single_cert(cert_chain, key)
    }
    .context("building TLS server config from device identity")?;
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

fn load_key(path: &str) -> anyhow::Result<rustls::pki_types::PrivateKeyDer<'static>> {
    let file = fs::File::open(path).with_context(|| format!("opening {path}"))?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)
        .with_context(|| format!("parsing PEM private key from {path}"))?
        .with_context(|| format!("no private key found in {path}"))
}
