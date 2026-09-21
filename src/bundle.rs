// SPDX-License-Identifier: Apache-2.0

//! One-click support bundle. Secrets, addresses, and hostnames are redacted
//! before anything is archived. The manifest can be previewed without writing
//! the archive.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use anyhow::Context;
use serde::Serialize;
use serde_json::{json, Value};

use crate::config::Config;
use crate::model::{DoctorReport, Inventory};
use crate::recorder;
use crate::signing::{self, SignatureBlock};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BundleFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleManifest {
    pub version: u32,
    pub since_unix_ms: u64,
    pub redacted: bool,
    pub files: Vec<BundleFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<SignatureBlock>,
}

#[derive(Clone)]
struct Entry {
    path: String,
    bytes: Vec<u8>,
}

pub fn redact(text: &str, hostname: &str) -> String {
    let mut out = text.to_string();
    if !hostname.is_empty() && hostname != "localhost" {
        out = out.replace(hostname, "[hostname]");
    }
    out = redact_pem_keys(&out);
    out = redact_macs(&out);
    out = redact_ipv4(&out);
    out
}

fn redact_pem_keys(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("-----BEGIN ") {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        if let Some(end_rel) = after.find("-----END ") {
            if let Some(tail) = after[end_rel..].find('\n') {
                out.push_str("[redacted-pem]\n");
                rest = &after[end_rel + tail + 1..];
                continue;
            }
            out.push_str("[redacted-pem]");
            return out;
        }
        out.push_str("[redacted-pem]");
        return out;
    }
    out.push_str(rest);
    out
}

fn redact_macs(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if let Some(len) = mac_len(&bytes[i..]) {
            out.push_str("[mac]");
            i += len;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn redact_ipv4(text: &str) -> String {
    let mut out = String::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if let Some(len) = ipv4_len(&bytes[i..]) {
            out.push_str("[ip]");
            i += len;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn ipv4_len(bytes: &[u8]) -> Option<usize> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut parts = 0usize;
    let mut index = 0usize;
    let chars: Vec<char> = text.chars().collect();
    while parts < 4 {
        let start = index;
        while index < chars.len() && chars[index].is_ascii_digit() {
            index += 1;
        }
        if index == start || index - start > 3 {
            return None;
        }
        let number: u32 = chars[start..index]
            .iter()
            .collect::<String>()
            .parse()
            .ok()?;
        if number > 255 {
            return None;
        }
        parts += 1;
        if parts == 4 {
            break;
        }
        if index >= chars.len() || chars[index] != '.' {
            return None;
        }
        index += 1;
    }
    if parts != 4 {
        return None;
    }
    Some(chars[..index].iter().collect::<String>().len())
}

fn mac_len(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 17 {
        return None;
    }
    let text = std::str::from_utf8(&bytes[..17]).ok()?;
    let chars: Vec<char> = text.chars().collect();
    if chars.len() != 17 {
        return None;
    }
    for (index, ch) in chars.iter().enumerate() {
        if index % 3 == 2 {
            if *ch != ':' && *ch != '-' {
                return None;
            }
        } else if !ch.is_ascii_hexdigit() {
            return None;
        }
    }
    Some(17)
}

pub fn preview(
    cfg: &Config,
    inventory: &Inventory,
    doctor: &DoctorReport,
    since_unix_ms: u64,
    redact_enabled: bool,
) -> anyhow::Result<BundleManifest> {
    let entries = collect(cfg, inventory, doctor, since_unix_ms, redact_enabled)?;
    Ok(manifest_for(cfg, since_unix_ms, redact_enabled, &entries))
}

pub fn write_archive(
    cfg: &Config,
    inventory: &Inventory,
    doctor: &DoctorReport,
    since_unix_ms: u64,
    redact_enabled: bool,
    output: &Path,
) -> anyhow::Result<BundleManifest> {
    let entries = collect(cfg, inventory, doctor, since_unix_ms, redact_enabled)?;
    let manifest = manifest_for(cfg, since_unix_ms, redact_enabled, &entries);
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(output).with_context(|| format!("creating {}", output.display()))?;
    let encoder = zstd::stream::Encoder::new(file, 3)?.auto_finish();
    let mut builder = tar::Builder::new(encoder);
    for entry in &entries {
        append_file(&mut builder, &entry.path, &entry.bytes)?;
    }
    append_file(&mut builder, "manifest.json", &manifest_bytes)?;
    builder.finish()?;
    Ok(manifest)
}

fn append_file<W: Write>(
    builder: &mut tar::Builder<W>,
    path: &str,
    bytes: &[u8],
) -> anyhow::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_path(path)?;
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append(&header, bytes)?;
    Ok(())
}

fn collect(
    cfg: &Config,
    inventory: &Inventory,
    doctor: &DoctorReport,
    since_unix_ms: u64,
    redact_enabled: bool,
) -> anyhow::Result<Vec<Entry>> {
    let hostname = inventory.device.hostname.clone();
    let passport = crate::passport::build(cfg, inventory, Some(doctor));
    let events =
        recorder::query(&recorder::directory(cfg), since_unix_ms, u64::MAX).unwrap_or_default();
    let mut files = vec![
        json_entry("passport.json", &passport)?,
        json_entry("doctor.json", doctor)?,
        json_entry("inventory.json", inventory)?,
        json_entry("recorder.json", &events)?,
        text_entry("config.toml", &config_text(cfg)),
        text_entry("service.txt", &service_text()),
        text_entry("network.txt", &network_text(inventory)),
        text_entry("can.txt", &can_text(inventory)),
        text_entry("certificate.txt", &certificate_metadata(cfg)),
        text_entry(
            "packages.txt",
            &format!("zyvor-device-agent {}\n", env!("CARGO_PKG_VERSION")),
        ),
    ];
    if redact_enabled {
        for entry in &mut files {
            if let Ok(text) = String::from_utf8(entry.bytes.clone()) {
                entry.bytes = redact(&text, &hostname).into_bytes();
            }
        }
    }
    Ok(files)
}

fn manifest_for(
    cfg: &Config,
    since_unix_ms: u64,
    redacted: bool,
    entries: &[Entry],
) -> BundleManifest {
    let files = entries
        .iter()
        .map(|entry| BundleFile {
            path: entry.path.clone(),
            sha256: signing::sha256_hex(&entry.bytes),
            bytes: entry.bytes.len() as u64,
        })
        .collect::<Vec<_>>();
    let mut manifest = BundleManifest {
        version: 1,
        since_unix_ms,
        redacted,
        files,
        signature: None,
    };
    let payload = serde_json::to_vec(&manifest).unwrap_or_default();
    manifest.signature = signing::sign_with_key_file(&cfg.auth.mtls.key_file, &payload)
        .ok()
        .flatten();
    manifest
}

fn json_entry<T: Serialize>(path: &str, value: &T) -> anyhow::Result<Entry> {
    Ok(Entry {
        path: path.into(),
        bytes: serde_json::to_vec_pretty(value)?,
    })
}

fn text_entry(path: &str, text: &str) -> Entry {
    Entry {
        path: path.into(),
        bytes: text.as_bytes().to_vec(),
    }
}

fn config_text(cfg: &Config) -> String {
    let mut value = serde_json::to_value(cfg).unwrap_or(Value::Null);
    scrub_config(&mut value);
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

fn scrub_config(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                let lower = key.to_ascii_lowercase();
                if lower.contains("password") || lower.contains("token") || lower.contains("key") {
                    *child = json!("[redacted]");
                } else {
                    scrub_config(child);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(scrub_config),
        _ => {}
    }
}

fn service_text() -> String {
    format!(
        "version={}\nuptime_hint=see passport\n",
        env!("CARGO_PKG_VERSION")
    )
}

fn network_text(inventory: &Inventory) -> String {
    inventory
        .network
        .iter()
        .map(|iface| {
            format!(
                "{} {} rx_errors={:?} tx_errors={:?}\n",
                iface.name, iface.operstate, iface.rx_errors, iface.tx_errors
            )
        })
        .collect()
}

fn can_text(inventory: &Inventory) -> String {
    inventory
        .industrial
        .can
        .iter()
        .map(|iface| {
            format!(
                "{} state={:?} rx_errors={:?} tx_errors={:?}\n",
                iface.name, iface.can_state, iface.rx_errors, iface.tx_errors
            )
        })
        .collect()
}

fn certificate_metadata(cfg: &Config) -> String {
    let path = &cfg.auth.mtls.cert_file;
    match fs::read(path) {
        Ok(pem) => match x509_parser::pem::parse_x509_pem(&pem) {
            Ok((_, pem)) => match pem.parse_x509() {
                Ok(cert) => format!(
                    "subject={}\nissuer={}\nnot_after={}\n",
                    cert.subject(),
                    cert.issuer(),
                    cert.validity().not_after
                ),
                Err(_) => "certificate present but unreadable\n".into(),
            },
            Err(_) => "certificate file is not a PEM certificate\n".into(),
        },
        Err(_) => "no device certificate\n".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_secrets_addresses_and_hostnames() {
        let raw = "host edge-gw ip 10.1.2.3 mac 02:42:ac:11:00:02\n-----BEGIN PRIVATE KEY-----\nsecret\n-----END PRIVATE KEY-----\n";
        let clean = redact(raw, "edge-gw");
        assert!(clean.contains("[hostname]"));
        assert!(clean.contains("[ip]"));
        assert!(clean.contains("[mac]"));
        assert!(clean.contains("[redacted-pem]"));
        assert!(!clean.contains("10.1.2.3"));
        assert!(!clean.contains("secret"));
        let fixture = include_str!("../fixtures/bundle/redaction.txt");
        let expected = include_str!("../fixtures/bundle/redaction.expected.txt");
        assert_eq!(redact(fixture, "edge-gw"), expected);
    }
}
