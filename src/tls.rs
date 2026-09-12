// SPDX-License-Identifier: Apache-2.0

//! `server.tls.enabled`: native TLS for the daemon's own TCP listener — not
//! a reverse-proxy config, and independent of `auth.mode` (unlike
//! `auth.mode = "mtls"`, this never requires a client certificate; it only
//! encrypts the connection and proves the daemon's own identity). If no
//! cert exists at `cert_path`/`key_path`, one is generated automatically on
//! first start so `https://` works with zero manual setup — mirrors
//! `../fabric`'s `zyvor-fabricd::tls` module. A real cert (from an internal
//! CA, ACME, etc.) can simply be mounted at those same paths instead.

use std::path::Path;

use anyhow::{Context, Result};

/// Best-effort discovery of this host's outbound IP, for the self-signed
/// cert's SAN list — doesn't actually send any packets (UDP `connect` only
/// resolves a route), so this works even fully offline, just less usefully
/// (falls back silently if it fails).
fn detect_local_ip() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|a| a.ip().to_string())
}

/// Generates a self-signed cert/key pair at `cert_path`/`key_path` if
/// either is missing. Leaves an existing cert/key alone — this only ever
/// fills a gap, never overwrites a real cert an operator put there.
pub fn ensure_self_signed_cert(cert_path: &str, key_path: &str) -> Result<()> {
    let cert_path = Path::new(cert_path);
    let key_path = Path::new(key_path);

    if cert_path.exists() && key_path.exists() {
        return Ok(());
    }

    tracing::info!(
        cert = %cert_path.display(),
        key = %key_path.display(),
        "no TLS cert found, generating a self-signed one"
    );

    let mut sans = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    if let Ok(hostname) = std::process::Command::new("hostname").output() {
        if let Ok(name) = String::from_utf8(hostname.stdout) {
            let name = name.trim();
            if !name.is_empty() {
                sans.push(name.to_string());
            }
        }
    }
    if let Some(ip) = detect_local_ip() {
        sans.push(ip);
    }

    let cert_key =
        rcgen::generate_simple_self_signed(sans).context("generating self-signed certificate")?;

    if let Some(parent) = cert_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    if let Some(parent) = key_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    std::fs::write(cert_path, cert_key.cert.pem())
        .with_context(|| format!("writing {}", cert_path.display()))?;
    std::fs::write(key_path, cert_key.signing_key.serialize_pem())
        .with_context(|| format!("writing {}", key_path.display()))?;

    // Private key: owner read/write only.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(key_path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("setting permissions on {}", key_path.display()))?;
    }

    tracing::info!("self-signed TLS certificate generated");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("zyvor-tls-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn generates_cert_and_key_when_missing() {
        let dir = test_dir("generates_cert_and_key_when_missing");
        let cert_path = dir.join("server.crt");
        let key_path = dir.join("server.key");

        ensure_self_signed_cert(cert_path.to_str().unwrap(), key_path.to_str().unwrap()).unwrap();

        assert!(cert_path.exists());
        assert!(key_path.exists());
        let cert_pem = std::fs::read_to_string(&cert_path).unwrap();
        assert!(cert_pem.contains("BEGIN CERTIFICATE"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn leaves_existing_cert_alone() {
        let dir = test_dir("leaves_existing_cert_alone");
        let cert_path = dir.join("server.crt");
        let key_path = dir.join("server.key");
        std::fs::write(&cert_path, "existing-cert").unwrap();
        std::fs::write(&key_path, "existing-key").unwrap();

        ensure_self_signed_cert(cert_path.to_str().unwrap(), key_path.to_str().unwrap()).unwrap();

        assert_eq!(std::fs::read_to_string(&cert_path).unwrap(), "existing-cert");
        assert_eq!(std::fs::read_to_string(&key_path).unwrap(), "existing-key");

        std::fs::remove_dir_all(&dir).ok();
    }
}
