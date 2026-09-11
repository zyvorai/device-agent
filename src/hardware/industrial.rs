// SPDX-License-Identifier: Apache-2.0

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::Value;

use crate::{
    config::IndustrialConfig,
    model::{BusInventory, CanInterfaceInfo, IndustrialInventory, Rs485Info, SerialPortInfo},
};

pub fn collect(cfg: &IndustrialConfig, raw_buses: &BusInventory) -> IndustrialInventory {
    IndustrialInventory {
        can: raw_buses
            .can
            .iter()
            .map(|name| collect_can(name, cfg))
            .collect(),
        serial: raw_buses
            .uart
            .iter()
            .map(|name| collect_serial(name, cfg))
            .collect(),
    }
}

fn collect_can(name: &str, cfg: &IndustrialConfig) -> CanInterfaceInfo {
    let base = PathBuf::from("/sys/class/net").join(name);
    let stats = base.join("statistics");
    let mut info = CanInterfaceInfo {
        name: name.to_string(),
        kind: if base.join("device").exists() {
            "physical"
        } else {
            "virtual"
        }
        .into(),
        operstate: read_trim(base.join("operstate")).unwrap_or_else(|| "unknown".into()),
        driver: driver_name(base.join("device/driver")),
        mtu: read_u64(base.join("mtu")).and_then(|value| value.try_into().ok()),
        rx_bytes: read_u64(stats.join("rx_bytes")),
        tx_bytes: read_u64(stats.join("tx_bytes")),
        rx_errors: read_u64(stats.join("rx_errors")),
        tx_errors: read_u64(stats.join("tx_errors")),
        rx_dropped: read_u64(stats.join("rx_dropped")),
        tx_dropped: read_u64(stats.join("tx_dropped")),
        details_source: "sysfs".into(),
        ..CanInterfaceInfo::default()
    };

    if !cfg.can_ip_command.trim().is_empty() {
        if let Some(details) = ip_can_details(&cfg.can_ip_command, name) {
            merge_ip_details(&mut info, &details);
            info.details_source = "iproute2+sysfs".into();
        }
    }
    info
}

fn collect_serial(name: &str, cfg: &IndustrialConfig) -> SerialPortInfo {
    let path = format!("/dev/{name}");
    let base = PathBuf::from("/sys/class/tty").join(name);
    let driver = driver_name(base.join("device/driver"));
    let transport = if name.starts_with("ttyUSB") || name.starts_with("ttyACM") {
        "usb"
    } else {
        "soc"
    };
    let declared = cfg
        .rs485_ports
        .iter()
        .any(|item| item == name || item == &path || item.trim_start_matches("/dev/") == name);
    let of_node = find_of_node(&base);
    let dt_seen = of_node
        .as_ref()
        .is_some_and(|node| rs485_property_present(node));

    let rs485 = if declared || dt_seen {
        let (before, after) = of_node
            .as_ref()
            .and_then(|node| read_be_u32_pair(node.join("rs485-rts-delay")))
            .unwrap_or((None, None));
        Some(Rs485Info {
            declared: true,
            source: match (declared, dt_seen) {
                (true, true) => "config+device-tree",
                (true, false) => "config",
                (false, true) => "device-tree",
                (false, false) => "unknown",
            }
            .into(),
            enabled_at_boot: of_node
                .as_ref()
                .map(|node| node.join("linux,rs485-enabled-at-boot-time").exists()),
            rts_active_high: of_node
                .as_ref()
                .map(|node| node.join("rs485-rts-active-high").exists()),
            rx_during_tx: of_node
                .as_ref()
                .map(|node| node.join("rs485-rx-during-tx").exists()),
            delay_before_send_ms: before,
            delay_after_send_ms: after,
        })
    } else {
        None
    };

    SerialPortInfo {
        name: name.to_string(),
        path,
        driver,
        transport: transport.into(),
        rs485,
    }
}

fn find_of_node(base: &Path) -> Option<PathBuf> {
    [
        base.join("device/of_node"),
        base.join("device/device/of_node"),
    ]
    .into_iter()
    .find(|path| path.exists())
}

fn rs485_property_present(node: &Path) -> bool {
    [
        "linux,rs485-enabled-at-boot-time",
        "rs485-rts-active-high",
        "rs485-rts-delay",
        "rs485-rx-during-tx",
    ]
    .iter()
    .any(|property| node.join(property).exists())
}

fn read_be_u32_pair(path: impl AsRef<Path>) -> Option<(Option<u32>, Option<u32>)> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() < 8 {
        return None;
    }
    let first = u32::from_be_bytes(bytes[0..4].try_into().ok()?);
    let second = u32::from_be_bytes(bytes[4..8].try_into().ok()?);
    Some((Some(first), Some(second)))
}

fn ip_can_details(command: &str, name: &str) -> Option<Value> {
    let output = Command::new(command)
        .args(["-j", "-details", "-statistics", "link", "show", "dev", name])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = serde_json::from_slice::<Value>(&output.stdout).ok()?;
    value.as_array()?.first().cloned()
}

fn merge_ip_details(info: &mut CanInterfaceInfo, value: &Value) {
    let data = value
        .get("linkinfo")
        .and_then(|v| v.get("info_data"))
        .unwrap_or(&Value::Null);

    info.can_state = data
        .get("state")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            value
                .get("operstate")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    info.bitrate = data
        .get("bittiming")
        .and_then(|v| v.get("bitrate"))
        .and_then(Value::as_u64);
    info.data_bitrate = data
        .get("data_bittiming")
        .and_then(|v| v.get("bitrate"))
        .and_then(Value::as_u64);
    info.restart_ms = data.get("restart_ms").and_then(Value::as_u64);
    info.tx_error_counter = data
        .get("berr_counter")
        .and_then(|v| v.get("tx"))
        .and_then(Value::as_u64);
    info.rx_error_counter = data
        .get("berr_counter")
        .and_then(|v| v.get("rx"))
        .and_then(Value::as_u64);
    info.controller_modes = parse_controller_modes(data.get("ctrlmode"));
}

fn parse_controller_modes(value: Option<&Value>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    let mut modes = match value {
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Value::Object(items) => items
            .iter()
            .filter(|(_, enabled)| enabled.as_bool().unwrap_or(false))
            .map(|(name, _)| name.clone())
            .collect(),
        Value::String(mode) => vec![mode.clone()],
        _ => Vec::new(),
    };
    modes.sort();
    modes
}

fn read_trim(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
}

fn read_u64(path: impl AsRef<Path>) -> Option<u64> {
    read_trim(path)?.parse().ok()
}

fn driver_name(path: impl AsRef<Path>) -> Option<String> {
    fs::read_link(path)
        .ok()?
        .file_name()?
        .to_str()
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_socketcan_json() {
        let raw = r#"[{"ifname":"can0","operstate":"UP","linkinfo":{"info_kind":"can","info_data":{"state":"ERROR-ACTIVE","bittiming":{"bitrate":500000},"data_bittiming":{"bitrate":2000000},"restart_ms":100,"berr_counter":{"tx":3,"rx":4},"ctrlmode":{"fd":true,"listen-only":false}}}}]"#;
        let value: Value = serde_json::from_str(raw).unwrap();
        let mut info = CanInterfaceInfo::default();
        merge_ip_details(&mut info, &value.as_array().unwrap()[0]);
        assert_eq!(info.can_state.as_deref(), Some("ERROR-ACTIVE"));
        assert_eq!(info.bitrate, Some(500_000));
        assert_eq!(info.data_bitrate, Some(2_000_000));
        assert_eq!(info.restart_ms, Some(100));
        assert_eq!(info.tx_error_counter, Some(3));
        assert_eq!(info.rx_error_counter, Some(4));
        assert_eq!(info.controller_modes, vec!["fd"]);
    }

    #[test]
    fn parses_device_tree_rs485_delay() {
        let dir = std::env::temp_dir().join(format!("zyvor-rs485-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("rs485-rts-delay");
        fs::write(&path, [0, 0, 0, 5, 0, 0, 0, 9]).unwrap();
        assert_eq!(read_be_u32_pair(&path), Some((Some(5), Some(9))));
        let _ = fs::remove_dir_all(&dir);
    }
}
