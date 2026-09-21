// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::model::Inventory;

/// Projection compatible with Zyvor Fleet's current Inventory schema.
/// The existing `fleet-agent` should merge this local hardware view into its
/// own heartbeat rather than Device Agent becoming a second fleet agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FleetInventoryProjection {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub kernel: String,
    pub cpu_count: usize,
    pub memory_bytes: u64,
    pub runtimes: Vec<String>,
    pub capabilities: Vec<String>,
    pub addresses: Vec<String>,
    pub metadata: BTreeMap<String, String>,
    /// Signed sibling of the projection. Older Fleet merges ignore it.
    #[serde(default)]
    pub signed: Option<SignedFleetInventory>,
}

pub fn project(inventory: &Inventory) -> FleetInventoryProjection {
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "zyvor.device.serial".into(),
        inventory.device.serial.clone(),
    );
    metadata.insert(
        "zyvor.device.vendor".into(),
        inventory.device.vendor.clone(),
    );
    metadata.insert("zyvor.device.model".into(), inventory.device.model.clone());
    metadata.insert(
        "zyvor.hardware.gpio".into(),
        inventory.buses.gpio_chips.len().to_string(),
    );
    metadata.insert(
        "zyvor.hardware.i2c".into(),
        inventory.buses.i2c.len().to_string(),
    );
    metadata.insert(
        "zyvor.hardware.spi".into(),
        inventory.buses.spi.len().to_string(),
    );
    metadata.insert(
        "zyvor.hardware.uart".into(),
        inventory.buses.uart.len().to_string(),
    );
    metadata.insert(
        "zyvor.hardware.can".into(),
        inventory.buses.can.len().to_string(),
    );
    metadata.insert("zyvor.hardware.usb".into(), inventory.usb.len().to_string());
    metadata.insert(
        "zyvor.hardware.watchdog".into(),
        inventory.buses.watchdog.len().to_string(),
    );
    metadata.insert(
        "zyvor.hardware.rs485".into(),
        inventory
            .industrial
            .serial
            .iter()
            .filter(|port| port.rs485.is_some())
            .count()
            .to_string(),
    );
    metadata.insert(
        "zyvor.hardware.can.detail".into(),
        inventory
            .industrial
            .can
            .iter()
            .map(|interface| {
                let state = interface
                    .can_state
                    .as_deref()
                    .unwrap_or(&interface.operstate);
                let bitrate = interface
                    .bitrate
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".into());
                format!("{}:{state}:{bitrate}", interface.name)
            })
            .collect::<Vec<_>>()
            .join(","),
    );
    metadata.insert(
        "zyvor.network.interfaces".into(),
        inventory
            .network
            .iter()
            .map(|n| n.name.as_str())
            .collect::<Vec<_>>()
            .join(","),
    );

    FleetInventoryProjection {
        hostname: inventory.device.hostname.clone(),
        os: inventory.system.os.clone(),
        arch: inventory.system.arch.clone(),
        kernel: inventory.system.kernel.clone(),
        cpu_count: inventory.system.cpu_cores,
        memory_bytes: inventory.system.memory_bytes,
        runtimes: vec![],
        capabilities: inventory.capabilities.clone(),
        addresses: inventory
            .network
            .iter()
            .flat_map(|interface| interface.addresses.clone())
            .collect(),
        metadata,
        signed: None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignedFleetInventory {
    pub device_id: String,
    pub boot_id: String,
    pub timestamp_unix_ms: u64,
    pub sequence: u64,
    pub digest_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<crate::signing::SignatureBlock>,
}

pub fn project_signed(
    inventory: &Inventory,
    cfg: &crate::config::Config,
) -> FleetInventoryProjection {
    let mut projection = project(inventory);
    let sequence = next_sequence(cfg);
    let boot_id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .unwrap_or_else(|_| "unknown".into())
        .trim()
        .to_string();
    let timestamp_unix_ms = crate::state::now_unix_ms();
    let digest_payload = serde_json::json!({
        "deviceId": inventory.device.serial,
        "bootId": boot_id,
        "timestampUnixMs": timestamp_unix_ms,
        "sequence": sequence,
        "hostname": projection.hostname,
        "capabilities": projection.capabilities,
    });
    let payload = serde_json::to_vec(&digest_payload).unwrap_or_default();
    let signature = crate::signing::sign_with_key_file(&cfg.auth.mtls.key_file, &payload)
        .ok()
        .flatten();
    projection.signed = Some(SignedFleetInventory {
        device_id: inventory.device.serial.clone(),
        boot_id,
        timestamp_unix_ms,
        sequence,
        digest_sha256: crate::signing::sha256_hex(&payload),
        signature,
    });
    projection
}

fn next_sequence(cfg: &crate::config::Config) -> u64 {
    let path = std::path::Path::new(&cfg.server.state_dir).join("fleet-sequence");
    let current = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| text.trim().parse::<u64>().ok())
        .unwrap_or(0);
    let next = current.saturating_add(1);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, next.to_string());
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::model::{DeviceIdentity, Inventory, SystemInfo};

    #[test]
    fn signed_projection_keeps_the_unsigned_fields() {
        let inventory = Inventory {
            device: DeviceIdentity {
                serial: "ZY-FLEET".into(),
                vendor: "Zyvor".into(),
                model: "edge".into(),
                hostname: "edge".into(),
                machine_id: "m".into(),
            },
            system: SystemInfo {
                arch: "aarch64".into(),
                kernel: "6.8".into(),
                os: "linux".into(),
                cpu_model: "cpu".into(),
                cpu_cores: 2,
                memory_bytes: 10,
                storage_bytes: None,
                uptime_seconds: 1,
            },
            network: vec![],
            buses: Default::default(),
            industrial: Default::default(),
            usb: vec![],
            thermal: vec![],
            capabilities: vec!["can".into()],
        };
        let mut cfg = Config::default();
        let dir = std::env::temp_dir().join(format!("zyvor-fleet-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        cfg.server.state_dir = dir.display().to_string();
        let key = rcgen::KeyPair::generate().unwrap();
        let key_path = dir.join("key.pem");
        std::fs::write(&key_path, key.serialize_pem()).unwrap();
        cfg.auth.mtls.key_file = key_path.display().to_string();
        let projection = project_signed(&inventory, &cfg);
        assert_eq!(projection.hostname, "edge");
        let signed = projection.signed.unwrap();
        assert_eq!(signed.device_id, "ZY-FLEET");
        assert_eq!(signed.sequence, 1);
        assert!(signed.signature.is_some());
        let again = project_signed(&inventory, &cfg);
        assert_eq!(again.signed.unwrap().sequence, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
