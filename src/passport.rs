// SPDX-License-Identifier: Apache-2.0

//! Versioned device passport. Fleet, Yard, and support consume this document.
//! It reuses the live inventory; it is not a second hardware model.

use std::fs;
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::model::{DoctorReport, Inventory};
use crate::signing::{self, SignatureBlock};

pub const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Passport {
    pub version: u32,
    pub device_id: String,
    pub boot_id: String,
    pub generated_at_unix_ms: u64,
    pub hardware: Inventory,
    pub profile: ProfileSummary,
    pub firmware: Firmware,
    pub tpm: TpmSummary,
    pub components: Vec<ComponentVersion>,
    pub config_digest: String,
    pub qualification: QualificationSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<SignatureBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub name: String,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Firmware {
    pub kernel: String,
    pub os: String,
    pub bios: String,
    pub bootloader: String,
    pub secure_boot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TpmSummary {
    pub policy: String,
    pub backend: String,
    pub available: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComponentVersion {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QualificationSummary {
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Serialize)]
struct SignedView<'a> {
    version: u32,
    device_id: &'a str,
    boot_id: &'a str,
    generated_at_unix_ms: u64,
    config_digest: &'a str,
    serial: &'a str,
    profile: &'a str,
    kernel: &'a str,
}

pub fn build(cfg: &Config, inventory: &Inventory, doctor: Option<&DoctorReport>) -> Passport {
    let profile_name = cfg.device.profile.clone();
    let confidence = profile_confidence(cfg, inventory);
    let qualification = match doctor {
        Some(report) if report.ok => QualificationSummary {
            status: "pass".into(),
            detail: format!("{} checks passed", report.checks.len()),
        },
        Some(report) => QualificationSummary {
            status: "fail".into(),
            detail: format!(
                "{} failing",
                report.checks.iter().filter(|check| !check.ok).count()
            ),
        },
        None => QualificationSummary {
            status: "unknown".into(),
            detail: "doctor has not been run".into(),
        },
    };
    let mut passport = Passport {
        version: VERSION,
        device_id: inventory.device.serial.clone(),
        boot_id: read_boot_id(),
        generated_at_unix_ms: crate::state::now_unix_ms(),
        hardware: inventory.clone(),
        profile: ProfileSummary {
            name: profile_name,
            confidence,
        },
        firmware: Firmware {
            kernel: inventory.system.kernel.clone(),
            os: inventory.system.os.clone(),
            bios: read_first_line("/sys/class/dmi/id/bios_version"),
            bootloader: read_first_line("/proc/sys/kernel/osrelease"),
            secure_boot: read_secure_boot(),
        },
        tpm: TpmSummary {
            policy: cfg.identity.policy.clone(),
            backend: cfg.identity.backend.clone(),
            available: tpm_available_label(cfg),
        },
        components: vec![ComponentVersion {
            name: "zyvor-device-agent".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }],
        config_digest: signing::sha256_hex(config_bytes(cfg).as_bytes()),
        qualification,
        signature: None,
    };
    if let Some(block) = sign_passport(cfg, &passport) {
        passport.signature = Some(block);
    }
    passport
}

pub fn canonical_payload(passport: &Passport) -> Vec<u8> {
    let view = SignedView {
        version: passport.version,
        device_id: &passport.device_id,
        boot_id: &passport.boot_id,
        generated_at_unix_ms: passport.generated_at_unix_ms,
        config_digest: &passport.config_digest,
        serial: &passport.hardware.device.serial,
        profile: &passport.profile.name,
        kernel: &passport.firmware.kernel,
    };
    serde_json::to_vec(&view).unwrap_or_default()
}

fn sign_passport(cfg: &Config, passport: &Passport) -> Option<SignatureBlock> {
    let payload = canonical_payload(passport);
    signing::sign_with_key_file(&cfg.auth.mtls.key_file, &payload).ok()?
}

pub fn verify_document(passport: &Passport) -> anyhow::Result<()> {
    let Some(signature) = &passport.signature else {
        anyhow::bail!("passport has no signature");
    };
    signing::verify(signature, &canonical_payload(passport))
}

pub fn verify_file(path: &Path) -> anyhow::Result<Passport> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let passport: Passport = serde_json::from_str(&raw).context("parsing passport JSON")?;
    verify_document(&passport)?;
    Ok(passport)
}

fn profile_confidence(cfg: &Config, inventory: &Inventory) -> u8 {
    let Ok(profile) = crate::profile::load(cfg) else {
        return 0;
    };
    let checks = [
        inventory.network.len() >= profile.minimum.ethernet,
        inventory.buses.gpio_chips.len() >= profile.minimum.gpio,
        inventory.buses.i2c.len() >= profile.minimum.i2c,
        inventory.buses.spi.len() >= profile.minimum.spi,
        inventory.buses.uart.len() >= profile.minimum.uart,
        inventory.buses.can.len() >= profile.minimum.can,
        inventory.usb.len() >= profile.minimum.usb,
        inventory.buses.watchdog.len() >= profile.minimum.watchdog,
    ];
    let met = checks.iter().filter(|ok| **ok).count();
    ((met * 100) / checks.len()) as u8
}

fn config_bytes(cfg: &Config) -> String {
    serde_json::to_string(cfg).unwrap_or_default()
}

fn read_boot_id() -> String {
    read_first_line("/proc/sys/kernel/random/boot_id")
}

fn read_secure_boot() -> String {
    let setup = read_first_line(
        "/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c",
    );
    if setup == "unknown" {
        "unknown".into()
    } else {
        "present".into()
    }
}

fn read_first_line(path: &str) -> String {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| text.lines().next().map(|line| line.trim().to_string()))
        .filter(|line| !line.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

fn tpm_available_label(cfg: &Config) -> String {
    if cfg.identity.policy == "software" && cfg.identity.backend != "tpm" {
        return "not-requested".into();
    }
    if Path::new("/dev/tpmrm0").exists() || Path::new("/dev/tpm0").exists() {
        "device-node".into()
    } else {
        "not-detected".into()
    }
}

pub fn to_pretty(passport: &Passport) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(passport)?)
}

/// Fixture-stable subset used by the unit test. Clock and host fields are
/// stripped so the golden file does not depend on the machine.
pub fn fixture_view(passport: &Passport) -> Value {
    serde_json::json!({
        "version": passport.version,
        "profile": passport.profile.name,
        "component": passport.components.first().map(|c| c.name.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DeviceIdentity, SystemInfo};

    fn inventory() -> Inventory {
        Inventory {
            device: DeviceIdentity {
                serial: "ZY-PASSPORT".into(),
                vendor: "Zyvor".into(),
                model: "fixture".into(),
                hostname: "edge".into(),
                machine_id: "abc".into(),
            },
            system: SystemInfo {
                arch: "aarch64".into(),
                kernel: "6.8.0".into(),
                os: "linux".into(),
                cpu_model: "test".into(),
                cpu_cores: 4,
                memory_bytes: 1024,
                storage_bytes: None,
                uptime_seconds: 10,
            },
            network: vec![],
            buses: Default::default(),
            industrial: Default::default(),
            usb: vec![],
            thermal: vec![],
            capabilities: vec!["hardware-inventory".into()],
        }
    }

    #[test]
    fn passport_fixture_shape_and_signature() {
        let mut cfg = Config::default();
        cfg.device.profile = "missing-profile".into();
        cfg.device.profile_directory = "profiles".into();
        let dir = std::env::temp_dir().join(format!("zyvor-passport-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let key = rcgen::KeyPair::generate().unwrap();
        let key_path = dir.join("device-key.pem");
        fs::write(&key_path, key.serialize_pem()).unwrap();
        cfg.auth.mtls.key_file = key_path.display().to_string();
        cfg.server.state_dir = dir.display().to_string();

        let passport = build(&cfg, &inventory(), None);
        assert_eq!(passport.version, 1);
        assert_eq!(passport.device_id, "ZY-PASSPORT");
        verify_document(&passport).unwrap();

        let view = fixture_view(&passport);
        let expected: Value =
            serde_json::from_str(include_str!("../fixtures/passport/passport-v1.json")).unwrap();
        assert_eq!(view, expected);

        let path = dir.join("passport.json");
        fs::write(&path, serde_json::to_string(&passport).unwrap()).unwrap();
        verify_file(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }
}
