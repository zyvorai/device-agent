// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

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
}

pub fn project(inventory: &Inventory) -> FleetInventoryProjection {
    let mut metadata = BTreeMap::new();
    metadata.insert("zyvor.device.serial".into(), inventory.device.serial.clone());
    metadata.insert("zyvor.device.vendor".into(), inventory.device.vendor.clone());
    metadata.insert("zyvor.device.model".into(), inventory.device.model.clone());
    metadata.insert("zyvor.hardware.gpio".into(), inventory.buses.gpio_chips.len().to_string());
    metadata.insert("zyvor.hardware.i2c".into(), inventory.buses.i2c.len().to_string());
    metadata.insert("zyvor.hardware.spi".into(), inventory.buses.spi.len().to_string());
    metadata.insert("zyvor.hardware.uart".into(), inventory.buses.uart.len().to_string());
    metadata.insert("zyvor.hardware.can".into(), inventory.buses.can.len().to_string());
    metadata.insert("zyvor.hardware.usb".into(), inventory.usb.len().to_string());
    metadata.insert("zyvor.hardware.watchdog".into(), inventory.buses.watchdog.len().to_string());
    metadata.insert("zyvor.network.interfaces".into(), inventory.network.iter().map(|n| n.name.as_str()).collect::<Vec<_>>().join(","));

    FleetInventoryProjection {
        hostname: inventory.device.hostname.clone(),
        os: inventory.system.os.clone(),
        arch: inventory.system.arch.clone(),
        kernel: inventory.system.kernel.clone(),
        cpu_count: inventory.system.cpu_cores,
        memory_bytes: inventory.system.memory_bytes,
        runtimes: vec![],
        capabilities: inventory.capabilities.clone(),
        addresses: vec![],
        metadata,
    }
}
