// SPDX-License-Identifier: Apache-2.0

use std::{fs, path::Path};

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub device: DeviceConfig,
    pub nodra: NodraConfig,
    pub fleet: FleetConfig,
    pub plugins: PluginConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub listen: String,
    pub dashboard_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceConfig {
    pub vendor: String,
    pub model: String,
    pub serial: String,
    pub profile: String,
    pub profile_directory: String,
    pub telemetry_interval_seconds: u64,
    pub inventory_refresh_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NodraConfig {
    pub enabled: bool,
    pub broker: String,
    pub port: u16,
    pub client_id: String,
    pub topic_prefix: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub retain_inventory: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FleetConfig {
    pub enabled: bool,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginConfig {
    pub directory: String,
    pub timeout_seconds: u64,
    pub sample_interval_seconds: u64,
    pub require_absolute_command: bool,
    pub reject_world_writable: bool,
    pub max_output_bytes: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0:9188".into(),
            dashboard_dir: "/usr/share/zyvor-device-agent/dashboard".into(),
        }
    }
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            vendor: "Generic Linux".into(),
            model: "Edge Gateway".into(),
            serial: "auto".into(),
            profile: "generic-linux-arm64".into(),
            profile_directory: "/etc/zyvor/device-agent/profiles".into(),
            telemetry_interval_seconds: 10,
            inventory_refresh_seconds: 5,
        }
    }
}

impl Default for NodraConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            broker: "127.0.0.1".into(),
            port: 1883,
            client_id: "zyvor-device-agent".into(),
            topic_prefix: "zyvor/device".into(),
            username: None,
            password: None,
            retain_inventory: true,
        }
    }
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: "projection".into(),
        }
    }
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            directory: "/etc/zyvor/device-agent/plugins.d".into(),
            timeout_seconds: 3,
            sample_interval_seconds: 5,
            require_absolute_command: true,
            reject_world_writable: true,
            max_output_bytes: 262_144,
        }
    }
}

impl Config {
    pub fn load_or_default(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
    }
}
