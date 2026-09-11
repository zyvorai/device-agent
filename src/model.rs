// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inventory {
    pub device: DeviceIdentity,
    pub system: SystemInfo,
    pub network: Vec<NetworkInterface>,
    pub buses: BusInventory,
    #[serde(default)]
    pub industrial: IndustrialInventory,
    pub usb: Vec<UsbDevice>,
    pub thermal: Vec<ThermalZone>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceIdentity {
    pub serial: String,
    pub vendor: String,
    pub model: String,
    pub hostname: String,
    pub machine_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub kind: String,
    pub operstate: String,
    pub mac: Option<String>,
    pub mtu: Option<u32>,
    #[serde(default)]
    pub addresses: Vec<String>,
    pub rx_bytes: Option<u64>,
    pub tx_bytes: Option<u64>,
    pub rx_errors: Option<u64>,
    pub tx_errors: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BusInventory {
    pub gpio_chips: Vec<String>,
    pub i2c: Vec<String>,
    pub spi: Vec<String>,
    pub uart: Vec<String>,
    pub can: Vec<String>,
    pub watchdog: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct IndustrialInventory {
    #[serde(default)]
    pub can: Vec<CanInterfaceInfo>,
    #[serde(default)]
    pub serial: Vec<SerialPortInfo>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CanInterfaceInfo {
    pub name: String,
    pub kind: String,
    pub operstate: String,
    pub driver: Option<String>,
    pub mtu: Option<u32>,
    pub bitrate: Option<u64>,
    pub data_bitrate: Option<u64>,
    pub can_state: Option<String>,
    pub restart_ms: Option<u64>,
    pub tx_error_counter: Option<u64>,
    pub rx_error_counter: Option<u64>,
    pub rx_bytes: Option<u64>,
    pub tx_bytes: Option<u64>,
    pub rx_errors: Option<u64>,
    pub tx_errors: Option<u64>,
    pub rx_dropped: Option<u64>,
    pub tx_dropped: Option<u64>,
    #[serde(default)]
    pub controller_modes: Vec<String>,
    pub details_source: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SerialPortInfo {
    pub name: String,
    pub path: String,
    pub driver: Option<String>,
    pub transport: String,
    pub rs485: Option<Rs485Info>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Rs485Info {
    pub declared: bool,
    pub source: String,
    pub enabled_at_boot: Option<bool>,
    pub rts_active_high: Option<bool>,
    pub rx_during_tx: Option<bool>,
    pub delay_before_send_ms: Option<u32>,
    pub delay_after_send_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanFrame {
    pub sequence: u64,
    pub interface: String,
    pub captured_at_unix_ms: u64,
    pub can_id: u32,
    pub extended: bool,
    pub remote: bool,
    pub error: bool,
    pub fd: bool,
    pub bitrate_switch: bool,
    pub error_state_indicator: bool,
    pub dlc: u8,
    #[serde(default)]
    pub data: Vec<u8>,
    pub data_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanCaptureStatus {
    pub enabled: bool,
    pub interfaces: Vec<String>,
    pub frames_total: u64,
    pub dropped_total: u64,
    pub decode_errors_total: u64,
    pub history_len: usize,
    pub subscribers: usize,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsbDevice {
    pub path: String,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    pub name: String,
    #[serde(default)]
    pub kind: String,
    pub value: f64,
    #[serde(default)]
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorSample {
    pub sensor_id: String,
    pub plugin: String,
    pub collected_at_unix_ms: u64,
    pub ok: bool,
    pub quality: String,
    pub publish_to_nodra: bool,
    #[serde(default)]
    pub readings: Vec<SensorReading>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    pub error: Option<String>,
    #[serde(default)]
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub id: u64,
    pub kind: String,
    pub at_unix_ms: u64,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub version: String,
    pub inventory_generation: u64,
    pub last_inventory_refresh_unix_ms: u64,
    pub sensor_samples_total: u64,
    pub sensor_sample_failures: u64,
    pub event_subscribers: usize,
    pub can_capture_frames_total: u64,
    pub can_capture_dropped_total: u64,
}
