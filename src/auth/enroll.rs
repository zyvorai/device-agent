// SPDX-License-Identifier: Apache-2.0

//! Client-side enrollment for `auth.mode = "mtls"`.
//!
//! Device Agent generates its own keypair and CSR locally (the private key never
//! leaves this function in plaintext form beyond the files written at the end),
//! submits the CSR plus a single-use enrollment token to `enrollment.server_url`,
//! and persists whatever certificate the server returns. There is deliberately no
//! CA or certificate-signing logic here: Fleet's enrollment token system today
//! issues an opaque bearer-style token, not a signed certificate, and building a
//! production CA (key custody, revocation, rotation) is a separate, security-
//! sensitive project of its own. This module only plays the client side of
//! whatever server implements the protocol below - see `docs/MTLS_ENROLLMENT.md`.
//!
//! Wire protocol (JSON over HTTPS):
//! `POST {server_url}` with `Authorization: Bearer <token>` and body
//! `{"csr_pem": "...", "common_name": "..."}`; a 200 response body of
//! `{"certificate_pem": "...", "ca_bundle_pem": "..."}` is treated as success.

use std::{fs, path::Path};

use anyhow::{bail, Context};
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::config::Config;

#[derive(Debug, Serialize)]
struct EnrollRequest {
    csr_pem: String,
    common_name: String,
}

#[derive(Debug, Deserialize)]
struct EnrollResponse {
    certificate_pem: String,
    #[serde(default)]
    ca_bundle_pem: String,
}

/// Runs the enrollment flow: generate keypair + CSR, submit to
/// `enrollment.server_url`, persist the issued certificate/key.
///
/// Refuses to run (without `force`) if a certificate already exists at
/// `auth.mtls.cert_file`, so a stray re-run can't silently clobber a working
/// identity - use `--force` for deliberate reissuance.
pub async fn run(cfg: &Config, force: bool) -> anyhow::Result<()> {
    if !cfg.enrollment.enabled {
        bail!("enrollment.enabled = false; nothing to do");
    }
    if cfg.enrollment.server_url.is_empty() {
        bail!("enrollment.server_url is not configured");
    }

    let cert_path = Path::new(&cfg.auth.mtls.cert_file);
    if cert_path.exists() && !force {
        bail!(
            "{} already exists; pass --force to reissue",
            cert_path.display()
        );
    }

    let token = fs::read_to_string(&cfg.enrollment.token_file)
        .with_context(|| {
            format!(
                "reading enrollment token from {}",
                cfg.enrollment.token_file
            )
        })?
        .trim()
        .to_string();
    if token.is_empty() {
        bail!(
            "enrollment token file {} is empty",
            cfg.enrollment.token_file
        );
    }

    let common_name = if cfg.enrollment.common_name.is_empty() {
        cfg.device.serial.clone()
    } else {
        cfg.enrollment.common_name.clone()
    };
    if common_name.is_empty() {
        bail!("enrollment.common_name is empty and device.serial is not set");
    }

    let key_pair = KeyPair::generate().context("generating device keypair")?;
    let mut params = CertificateParams::new(Vec::new()).context("building CSR parameters")?;
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, common_name.as_str());
    params.distinguished_name = dn;
    let csr = params
        .serialize_request(&key_pair)
        .context("serializing CSR")?;
    let csr_pem = csr.pem().context("PEM-encoding CSR")?;

    let mut client_builder = reqwest::Client::builder();
    if !cfg.enrollment.ca_bundle_file.is_empty() {
        let ca_pem = fs::read(&cfg.enrollment.ca_bundle_file).with_context(|| {
            format!(
                "reading enrollment.ca_bundle_file {}",
                cfg.enrollment.ca_bundle_file
            )
        })?;
        let ca_cert =
            reqwest::Certificate::from_pem(&ca_pem).context("parsing enrollment.ca_bundle_file")?;
        client_builder = client_builder.add_root_certificate(ca_cert);
    }
    let client = client_builder.build().context("building HTTPS client")?;

    info!(url = %cfg.enrollment.server_url, %common_name, "submitting CSR for enrollment");
    let response = client
        .post(&cfg.enrollment.server_url)
        .bearer_auth(&token)
        .json(&EnrollRequest {
            csr_pem,
            common_name: common_name.clone(),
        })
        .send()
        .await
        .context("submitting enrollment request")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!("enrollment server returned {status}: {body}");
    }
    let enrolled: EnrollResponse = response
        .json()
        .await
        .context("parsing enrollment server response")?;

    write_identity_file(
        &cfg.auth.mtls.cert_file,
        enrolled.certificate_pem.as_bytes(),
    )?;
    write_identity_file(&cfg.auth.mtls.key_file, key_pair.serialize_pem().as_bytes())?;
    if !enrolled.ca_bundle_pem.is_empty() && !cfg.auth.mtls.client_ca_file.is_empty() {
        write_identity_file(
            &cfg.auth.mtls.client_ca_file,
            enrolled.ca_bundle_pem.as_bytes(),
        )?;
    }

    // Single-use: the server should already reject a replayed token, but
    // removing it locally means a compromised disk image can't accidentally
    // hand out a live token from a previous enrollment.
    if let Err(error) = fs::remove_file(&cfg.enrollment.token_file) {
        if error.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(%error, "failed to remove enrollment token after successful enrollment");
        }
    }

    info!(
        cert_file = %cfg.auth.mtls.cert_file,
        key_file = %cfg.auth.mtls.key_file,
        "enrollment complete"
    );
    Ok(())
}

fn write_identity_file(path: &str, contents: &[u8]) -> anyhow::Result<()> {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("writing {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Private keys and issued certs alike: readable only by the owner
        // (expected to be the daemon's own uid, today root).
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("setting permissions on {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn refuses_when_enrollment_disabled() {
        let cfg = Config::default();
        let error = run(&cfg, false).await.unwrap_err();
        assert!(error.to_string().contains("enrollment.enabled"));
    }

    #[tokio::test]
    async fn refuses_without_server_url() {
        let mut cfg = Config::default();
        cfg.enrollment.enabled = true;
        let error = run(&cfg, false).await.unwrap_err();
        assert!(error.to_string().contains("server_url"));
    }

    #[tokio::test]
    async fn refuses_to_reissue_without_force() {
        let dir = std::env::temp_dir().join(format!(
            "zyvor-enroll-test-{}-{}",
            std::process::id(),
            "refuses_to_reissue_without_force"
        ));
        fs::create_dir_all(&dir).unwrap();
        let cert_path = dir.join("device-cert.pem");
        fs::write(&cert_path, b"placeholder").unwrap();

        let mut cfg = Config::default();
        cfg.enrollment.enabled = true;
        cfg.enrollment.server_url = "https://example.invalid/enroll".into();
        cfg.auth.mtls.cert_file = cert_path.to_string_lossy().into_owned();

        let error = run(&cfg, false).await.unwrap_err();
        assert!(error.to_string().contains("--force"));

        fs::remove_dir_all(&dir).ok();
    }
}
