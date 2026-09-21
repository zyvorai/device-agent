// SPDX-License-Identifier: Apache-2.0

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardProfile {
    pub name: String,
    pub vendor: String,
    pub arch: String,
    pub minimum: MinimumInterfaces,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinimumInterfaces {
    pub ethernet: usize,
    pub gpio: usize,
    pub i2c: usize,
    pub spi: usize,
    pub uart: usize,
    pub can: usize,
    pub usb: usize,
    pub watchdog: usize,
}

pub fn load(cfg: &Config) -> anyhow::Result<BoardProfile> {
    let directory = PathBuf::from(&cfg.device.profile_directory);
    let nested = directory.join(&cfg.device.profile).join("profile.toml");
    let flat = directory.join(format!("{}.toml", cfg.device.profile));
    let path = if nested.is_file() { nested } else { flat };
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("reading board profile {}", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("parsing board profile {}", path.display()))
}

pub fn profile_dir(cfg: &Config) -> PathBuf {
    PathBuf::from(&cfg.device.profile_directory).join(&cfg.device.profile)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QualificationReport {
    pub profile: String,
    pub status: String,
    pub expected: MinimumInterfaces,
    pub detected: MinimumInterfaces,
    pub kernel: String,
    pub profile_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<crate::signing::SignatureBlock>,
}

pub fn qualify(
    cfg: &Config,
    inventory: &crate::model::Inventory,
) -> anyhow::Result<QualificationReport> {
    let profile = load(cfg)?;
    let detected = MinimumInterfaces {
        ethernet: inventory.network.len(),
        gpio: inventory.buses.gpio_chips.len(),
        i2c: inventory.buses.i2c.len(),
        spi: inventory.buses.spi.len(),
        uart: inventory.buses.uart.len(),
        can: inventory.buses.can.len(),
        usb: inventory.usb.len(),
        watchdog: inventory.buses.watchdog.len(),
    };
    let pass = detected.ethernet >= profile.minimum.ethernet
        && detected.gpio >= profile.minimum.gpio
        && detected.i2c >= profile.minimum.i2c
        && detected.spi >= profile.minimum.spi
        && detected.uart >= profile.minimum.uart
        && detected.can >= profile.minimum.can
        && detected.usb >= profile.minimum.usb
        && detected.watchdog >= profile.minimum.watchdog;
    let directory = PathBuf::from(&cfg.device.profile_directory);
    let nested = directory.join(&cfg.device.profile).join("profile.toml");
    let flat = directory.join(format!("{}.toml", cfg.device.profile));
    let path = if nested.is_file() { nested } else { flat };
    let raw = fs::read(&path).unwrap_or_default();
    let mut report = QualificationReport {
        profile: profile.name,
        status: if pass { "pass" } else { "fail" }.into(),
        expected: profile.minimum,
        detected,
        kernel: inventory.system.kernel.clone(),
        profile_sha256: crate::signing::sha256_hex(&raw),
        signature: None,
    };
    let payload = serde_json::to_vec(&report).unwrap_or_default();
    report.signature = crate::signing::sign_with_key_file(&cfg.auth.mtls.key_file, &payload)
        .ok()
        .flatten();
    Ok(report)
}

pub fn package_profile(cfg: &Config, output: &std::path::Path) -> anyhow::Result<()> {
    let dir = profile_dir(cfg);
    let source = if dir.join("profile.toml").is_file() {
        dir
    } else {
        PathBuf::from(&cfg.device.profile_directory)
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(output)?;
    let mut builder = tar::Builder::new(file);
    if source.is_dir() && source.join("profile.toml").is_file() {
        builder.append_dir_all(cfg.device.profile.as_str(), &source)?;
    } else {
        let flat = source.join(format!("{}.toml", cfg.device.profile));
        let bytes = fs::read(&flat).with_context(|| format!("reading {}", flat.display()))?;
        let mut header = tar::Header::new_gnu();
        header.set_path(format!("{}.toml", cfg.device.profile))?;
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, bytes.as_slice())?;
    }
    builder.finish()?;
    Ok(())
}
