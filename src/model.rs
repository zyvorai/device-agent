// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    pub device: DeviceIdentity,
    pub system: SystemInfo,
    pub network: Vec<NetworkInterface>,
    pub buses: BusInventory,
    pub usb: Vec<UsbDevice>,
    pub thermal: Vec<ThermalZone>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceIdentity {
    pub serial: String,
    pub vendor: String,
    pub model: String,
    pub hostname: String,
    pub machine_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub arch: String,
    pub kernel: String,
    pub os: String,
    pub cpu_model: String,
    pub cpu_cores: usize,
    pub memory_bytes: u64,
    pub storage_bytes: Option<u64>,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub kind: String,
    pub operstate: String,
    pub mac: Option<String>,
    pub mtu: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BusInventory {
    pub gpio_chips: Vec<String>,
    pub i2c: Vec<String>,
    pub spi: Vec<String>,
    pub uart: Vec<String>,
    pub can: Vec<String>,
    pub watchdog: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbDevice {
    pub path: String,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalZone {
    pub name: String,
    pub kind: String,
    pub celsius: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub checks: Vec<DoctorCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationStatus {
    pub nodra_enabled: bool,
    pub nodra_connected: bool,
    pub fleet_enabled: bool,
    pub fleet_projection_ready: bool,
}
