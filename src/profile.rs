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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    let mut path = PathBuf::from(&cfg.device.profile_directory);
    path.push(format!("{}.toml", cfg.device.profile));
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("reading board profile {}", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("parsing board profile {}", path.display()))
}
