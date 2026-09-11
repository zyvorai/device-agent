// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, fs, path::Path, process::Command};

use crate::model::NetworkInterface;

fn read(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path).ok().map(|v| v.trim().to_string())
}

fn read_u64(path: impl AsRef<Path>) -> Option<u64> {
    read(path)?.parse().ok()
}

fn collect_addresses() -> BTreeMap<String, Vec<String>> {
    let Ok(output) = Command::new("ip").args(["-j", "address", "show"]).output() else {
        return BTreeMap::new();
    };
    if !output.status.success() {
        return BTreeMap::new();
    }

    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return BTreeMap::new();
    };
    let Some(items) = value.as_array() else {
        return BTreeMap::new();
    };

    let mut addresses = BTreeMap::new();
    for item in items {
        let Some(name) = item.get("ifname").and_then(|v| v.as_str()) else {
            continue;
        };
        let mut found = Vec::new();
        if let Some(info) = item.get("addr_info").and_then(|v| v.as_array()) {
            for address in info {
                let Some(local) = address.get("local").and_then(|v| v.as_str()) else {
                    continue;
                };
                let prefix = address
                    .get("prefixlen")
                    .and_then(|v| v.as_u64())
                    .map(|v| format!("/{v}"))
                    .unwrap_or_default();
                found.push(format!("{local}{prefix}"));
            }
        }
        found.sort();
        found.dedup();
        addresses.insert(name.to_string(), found);
    }
    addresses
}

pub fn collect() -> Vec<NetworkInterface> {
    let mut out = Vec::new();
    let addresses = collect_addresses();
    let Ok(entries) = fs::read_dir("/sys/class/net") else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let base = entry.path();
        let kind = if name == "lo" {
            "loopback"
        } else if base.join("wireless").exists() || name.starts_with("wl") {
            "wifi"
        } else if name.starts_with("can") || name.starts_with("vcan") {
            "can"
        } else {
            "ethernet"
        };
        let stats = base.join("statistics");
        out.push(NetworkInterface {
            addresses: addresses.get(&name).cloned().unwrap_or_default(),
            name,
            kind: kind.into(),
            operstate: read(base.join("operstate")).unwrap_or_else(|| "unknown".into()),
            mac: read(base.join("address")),
            mtu: read(base.join("mtu")).and_then(|v| v.parse().ok()),
            rx_bytes: read_u64(stats.join("rx_bytes")),
            tx_bytes: read_u64(stats.join("tx_bytes")),
            rx_errors: read_u64(stats.join("rx_errors")),
            tx_errors: read_u64(stats.join("tx_errors")),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}
