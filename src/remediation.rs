// SPDX-License-Identifier: Apache-2.0

//! Signed, allowlisted remediation. There is no shell and no arbitrary exec.
//! Disruptive actions can require a second signature.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use p256::ecdsa::signature::{Signer, Verifier};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::pkcs8::{DecodePrivateKey, DecodePublicKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::{Config, RemediationConfig};

pub const ACTIONS: &[&str] = &[
    "refresh-inventory",
    "run-diagnostic",
    "restart-service",
    "reset-plugin",
    "reconnect-nodra",
    "rotate-certificate",
    "capture-support-bundle",
    "reopen-can",
    "reboot",
    "request-ota",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub action: String,
    pub expires_at_unix_ms: u64,
    pub nonce: String,
    #[serde(default)]
    pub payload: Value,
    pub signature_hex: String,
    #[serde(default)]
    pub countersignature_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub action: String,
    pub ok: bool,
    pub before: Value,
    pub after: Value,
    pub detail: String,
}

#[derive(Serialize)]
struct SignedBody<'a> {
    action: &'a str,
    expires_at_unix_ms: u64,
    nonce: &'a str,
    payload: &'a Value,
}

pub fn canonical(command: &Command) -> Vec<u8> {
    let body = SignedBody {
        action: &command.action,
        expires_at_unix_ms: command.expires_at_unix_ms,
        nonce: &command.nonce,
        payload: &command.payload,
    };
    serde_json::to_vec(&body).unwrap_or_default()
}

pub fn sign(command: &Command, private_key_pem: &str) -> anyhow::Result<String> {
    let key = SigningKey::from_pkcs8_pem(private_key_pem)?;
    let signature: Signature = key.sign(&canonical(command));
    Ok(crate::signing::hex_encode(&signature.to_bytes()))
}

pub fn execute(
    cfg: &Config,
    command: &Command,
    now_unix_ms: u64,
    replay: &mut BTreeSet<String>,
) -> anyhow::Result<ActionResult> {
    if !cfg.remediation.enabled {
        anyhow::bail!("remediation is disabled");
    }
    if !ACTIONS.contains(&command.action.as_str()) {
        anyhow::bail!("action {} is not allowlisted", command.action);
    }
    if command.expires_at_unix_ms < now_unix_ms {
        anyhow::bail!("command expired");
    }
    if command.nonce.is_empty() || !replay.insert(command.nonce.clone()) {
        anyhow::bail!("command nonce was already used");
    }
    verify_signature(
        &cfg.remediation,
        &command.signature_hex,
        &canonical(command),
    )?;
    if needs_second_signature(&cfg.remediation, &command.action) {
        if command.countersignature_hex.is_empty() {
            anyhow::bail!("{} requires a second signature", command.action);
        }
        if command.countersignature_hex == command.signature_hex {
            anyhow::bail!("countersignature must be from a second party");
        }
        verify_signature(
            &cfg.remediation,
            &command.countersignature_hex,
            &canonical(command),
        )?;
    }
    authorize_payload(&cfg.remediation, command)?;
    let before = json!({"action": command.action, "payload": command.payload});
    let detail = plan_effect(command);
    Ok(ActionResult {
        action: command.action.clone(),
        ok: true,
        before,
        after: json!({"accepted": true, "effect": detail}),
        detail,
    })
}

fn needs_second_signature(cfg: &RemediationConfig, action: &str) -> bool {
    cfg.two_person && matches!(action, "reboot" | "reopen-can")
}

fn verify_signature(
    cfg: &RemediationConfig,
    signature_hex: &str,
    payload: &[u8],
) -> anyhow::Result<()> {
    if cfg.fleet_public_key_pem.is_empty() {
        anyhow::bail!("remediation.fleet_public_key_pem is empty");
    }
    let verifying = VerifyingKey::from_public_key_pem(cfg.fleet_public_key_pem.as_str())
        .context("parsing Fleet public key")?;
    let bytes = crate::signing::hex_decode(signature_hex)?;
    let signature = Signature::from_slice(&bytes).context("signature")?;
    verifying
        .verify(payload, &signature)
        .context("command signature rejected")?;
    Ok(())
}

fn authorize_payload(cfg: &RemediationConfig, command: &Command) -> anyhow::Result<()> {
    match command.action.as_str() {
        "restart-service" => {
            let unit = command
                .payload
                .get("unit")
                .and_then(Value::as_str)
                .unwrap_or("");
            if !cfg.service_allowlist.iter().any(|allowed| allowed == unit) {
                anyhow::bail!("systemd unit {unit} is not allowlisted");
            }
        }
        "reopen-can" => {
            let iface = command
                .payload
                .get("interface")
                .and_then(Value::as_str)
                .unwrap_or("");
            if !cfg.can_allowlist.iter().any(|allowed| allowed == iface) {
                anyhow::bail!("CAN interface {iface} is not allowlisted");
            }
        }
        "request-ota"
            if command
                .payload
                .get("artifact")
                .and_then(Value::as_str)
                .is_none() =>
        {
            anyhow::bail!(
                "OTA request requires an artifact name; Device Agent does not write boot slots"
            );
        }
        "request-ota" => {}
        _ => {}
    }
    Ok(())
}

fn plan_effect(command: &Command) -> String {
    match command.action.as_str() {
        "refresh-inventory" => "inventory refresh requested".into(),
        "run-diagnostic" => "doctor requested".into(),
        "restart-service" => "approved service restart requested; no shell".into(),
        "reset-plugin" => "plugin reset requested".into(),
        "reconnect-nodra" => "Nodra reconnect requested".into(),
        "rotate-certificate" => "certificate rotation requested via enrollment".into(),
        "capture-support-bundle" => "support bundle requested".into(),
        "reopen-can" => "allowlisted CAN interface reopen requested; no transmit".into(),
        "reboot" => "reboot accepted by policy; not performed by this function".into(),
        "request-ota" => {
            "signed OTA request recorded for Zyvor OTA; boot slots were not written".into()
        }
        other => format!("{other} accepted"),
    }
}

pub fn replay_path(cfg: &Config) -> PathBuf {
    PathBuf::from(&cfg.server.state_dir).join("remediation-nonces.txt")
}

pub fn load_replay(cfg: &Config) -> BTreeSet<String> {
    fs::read_to_string(replay_path(cfg))
        .map(|text| text.lines().map(|line| line.to_string()).collect())
        .unwrap_or_default()
}

pub fn store_replay(cfg: &Config, replay: &BTreeSet<String>) -> anyhow::Result<()> {
    let path = replay_path(cfg);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, replay.iter().cloned().collect::<Vec<_>>().join("\n"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_refresh_is_accepted_and_replay_is_rejected() {
        let (private, public) = test_keypair();
        let mut cfg = Config::default();
        cfg.remediation.enabled = true;
        cfg.remediation.two_person = true;
        cfg.remediation.fleet_public_key_pem = public;
        let mut command = Command {
            action: "refresh-inventory".into(),
            expires_at_unix_ms: 9_000,
            nonce: "n-1".into(),
            payload: json!({}),
            signature_hex: String::new(),
            countersignature_hex: String::new(),
        };
        command.signature_hex = sign(&command, &private).unwrap();
        let mut replay = BTreeSet::new();
        let result = execute(&cfg, &command, 1_000, &mut replay).unwrap();
        assert!(result.ok);
        assert!(result.detail.contains("inventory"));
        let error = execute(&cfg, &command, 1_000, &mut replay).unwrap_err();
        assert!(error.to_string().contains("nonce"));
    }

    #[test]
    fn reboot_requires_a_distinct_countersignature() {
        let (private, public) = test_keypair();
        let mut cfg = Config::default();
        cfg.remediation.enabled = true;
        cfg.remediation.fleet_public_key_pem = public;
        let mut command = Command {
            action: "reboot".into(),
            expires_at_unix_ms: 9_000,
            nonce: "n-2".into(),
            payload: json!({}),
            signature_hex: String::new(),
            countersignature_hex: String::new(),
        };
        command.signature_hex = sign(&command, &private).unwrap();
        let mut replay = BTreeSet::new();
        let error = execute(&cfg, &command, 1_000, &mut replay).unwrap_err();
        assert!(error.to_string().contains("second signature"));
    }

    #[test]
    fn shell_is_not_an_action() {
        let fixture = include_str!("../fixtures/remediation/refresh.json");
        let command: Command = serde_json::from_str(fixture).unwrap();
        assert_eq!(command.action, "refresh-inventory");
        assert!(!ACTIONS.contains(&"sh"));
    }

    fn test_keypair() -> (String, String) {
        use p256::pkcs8::EncodePublicKey;
        let generated = rcgen::KeyPair::generate().unwrap();
        let signing = SigningKey::from_pkcs8_pem(&generated.serialize_pem()).unwrap();
        let public = signing
            .verifying_key()
            .to_public_key_pem(p256::pkcs8::LineEnding::LF)
            .unwrap()
            .to_string();
        (generated.serialize_pem(), public)
    }
}
