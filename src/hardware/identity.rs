// SPDX-License-Identifier: Apache-2.0

use crate::{config::Config, model::DeviceIdentity};
use std::fs;

fn read_trim(path: &str) -> String {
    fs::read_to_string(path)
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub fn collect(cfg: &Config) -> DeviceIdentity {
    let machine_id = read_trim("/etc/machine-id");
    let hostname = read_trim("/etc/hostname");
    let serial = if cfg.device.serial == "auto" {
        let dt = read_trim("/sys/firmware/devicetree/base/serial-number");
        if !dt.is_empty() {
            dt.trim_matches(char::from(0)).to_string()
        } else if !machine_id.is_empty() {
            format!(
                "ZY-{}",
                machine_id[..machine_id.len().min(12)].to_uppercase()
            )
        } else {
            "ZY-UNSET".into()
        }
    } else {
        cfg.device.serial.clone()
    };

    DeviceIdentity {
        serial,
        vendor: cfg.device.vendor.clone(),
        model: cfg.device.model.clone(),
        hostname,
        machine_id,
    }
}
