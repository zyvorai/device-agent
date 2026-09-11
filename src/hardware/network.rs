// SPDX-License-Identifier: Apache-2.0

use std::{fs, path::Path};
use crate::model::NetworkInterface;

fn read(path: impl AsRef<Path>) -> Option<String> { fs::read_to_string(path).ok().map(|v| v.trim().to_string()) }

pub fn collect() -> Vec<NetworkInterface> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/net") else { return out; };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let base = entry.path();
        let kind = if name == "lo" { "loopback" } else if base.join("wireless").exists() || name.starts_with("wl") { "wifi" } else if name.starts_with("can") { "can" } else { "ethernet" };
        out.push(NetworkInterface {
            name,
            kind: kind.into(),
            operstate: read(base.join("operstate")).unwrap_or_else(|| "unknown".into()),
            mac: read(base.join("address")),
            mtu: read(base.join("mtu")).and_then(|v| v.parse().ok()),
        });
    }
    out.sort_by(|a,b| a.name.cmp(&b.name));
    out
}
