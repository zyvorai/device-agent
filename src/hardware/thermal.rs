// SPDX-License-Identifier: Apache-2.0

use crate::model::ThermalZone;
use std::fs;

pub fn collect() -> Vec<ThermalZone> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/thermal") else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("thermal_zone") {
            continue;
        }
        let base = entry.path();
        let kind = fs::read_to_string(base.join("type"))
            .unwrap_or_default()
            .trim()
            .to_string();
        let celsius = fs::read_to_string(base.join("temp"))
            .ok()
            .and_then(|v| v.trim().parse::<f64>().ok())
            .map(|v| if v > 1000.0 { v / 1000.0 } else { v });
        out.push(ThermalZone {
            name,
            kind,
            celsius,
        });
    }
    out
}
