// SPDX-License-Identifier: Apache-2.0

mod buses;
pub mod hotplug;
mod identity;
mod industrial;
mod network;
mod system;
mod thermal;
mod usb;

use crate::{
    config::Config,
    model::{DoctorCheck, DoctorReport, IndustrialInventory, Inventory},
    profile,
};

pub async fn collect_inventory(cfg: &Config) -> Inventory {
    let device = identity::collect(cfg);
    let system = system::collect();
    let network = network::collect();
    let buses = buses::collect();
    let industrial = industrial::collect(&cfg.industrial, &buses);
    let usb = usb::collect();
    let thermal = thermal::collect();

    let mut capabilities = vec![
        "hardware-inventory".into(),
        "system-health".into(),
        "sensor-plugin-api".into(),
        "sensor-scheduler".into(),
        "hardware-events".into(),
        "fleet-inventory-projection".into(),
        "industrial-inventory".into(),
    ];
    if !buses.gpio_chips.is_empty() {
        capabilities.push("gpio".into());
    }
    if !buses.i2c.is_empty() {
        capabilities.push("i2c".into());
    }
    if !buses.spi.is_empty() {
        capabilities.push("spi".into());
    }
    if !buses.uart.is_empty() {
        capabilities.push("uart".into());
        capabilities.push("rs485-awareness".into());
    }
    if !buses.can.is_empty() {
        capabilities.push("can".into());
        capabilities.push("can-health".into());
    }
    if industrial.serial.iter().any(|port| port.rs485.is_some()) {
        capabilities.push("rs485-declared".into());
    }
    if !buses.watchdog.is_empty() {
        capabilities.push("watchdog".into());
    }
    if cfg.industrial.can_capture.enabled {
        capabilities.push("can-capture-readonly".into());
    }

    Inventory {
        device,
        system,
        network,
        buses,
        industrial,
        usb,
        thermal,
        capabilities,
    }
}

pub async fn industrial_inventory(cfg: &Config) -> IndustrialInventory {
    let buses = buses::collect();
    industrial::collect(&cfg.industrial, &buses)
}

pub async fn doctor(cfg: &Config) -> DoctorReport {
    let inventory = collect_inventory(cfg).await;
    let mut checks = Vec::new();
    checks.push(DoctorCheck {
        name: "machine-id".into(),
        ok: !inventory.device.machine_id.is_empty(),
        detail: inventory.device.machine_id.clone(),
    });
    checks.push(DoctorCheck {
        name: "device-serial".into(),
        ok: inventory.device.serial != "ZY-UNSET",
        detail: inventory.device.serial.clone(),
    });
    checks.push(DoctorCheck {
        name: "network".into(),
        ok: !inventory.network.is_empty(),
        detail: format!("{} interfaces", inventory.network.len()),
    });
    checks.push(DoctorCheck {
        name: "network-physical".into(),
        ok: inventory
            .network
            .iter()
            .any(|interface| interface.name != "lo"),
        detail: inventory
            .network
            .iter()
            .filter(|interface| interface.name != "lo")
            .map(|interface| format!("{}:{}", interface.name, interface.operstate))
            .collect::<Vec<_>>()
            .join(", "),
    });
    checks.push(DoctorCheck {
        name: "thermal".into(),
        ok: true,
        detail: format!("{} thermal zones", inventory.thermal.len()),
    });
    checks.push(DoctorCheck {
        name: "plugin-directory".into(),
        ok: true,
        detail: cfg.plugins.directory.clone(),
    });

    for can in &inventory.industrial.can {
        let state = can.can_state.as_deref().unwrap_or(&can.operstate);
        let healthy = !state.eq_ignore_ascii_case("bus-off");
        let bitrate = can
            .bitrate
            .map(|value| format!("{value} bps"))
            .unwrap_or_else(|| "bitrate unknown".into());
        checks.push(DoctorCheck {
            name: format!("can.{}", can.name),
            ok: healthy,
            detail: format!(
                "{} · {} · {} · controller-errors tx={} rx={} · net-errors tx={} rx={}",
                can.kind,
                state,
                bitrate,
                can.tx_error_counter.unwrap_or(0),
                can.rx_error_counter.unwrap_or(0),
                can.tx_errors.unwrap_or(0),
                can.rx_errors.unwrap_or(0)
            ),
        });
    }

    for declared in &cfg.industrial.rs485_ports {
        let normalized = declared.trim_start_matches("/dev/");
        let port = inventory
            .industrial
            .serial
            .iter()
            .find(|port| port.name == normalized);
        checks.push(DoctorCheck {
            name: format!("rs485.{normalized}"),
            ok: port.is_some(),
            detail: port
                .map(|port| {
                    let source = port
                        .rs485
                        .as_ref()
                        .map(|rs485| rs485.source.as_str())
                        .unwrap_or("configured");
                    format!(
                        "{} · driver={} · source={source}",
                        port.path,
                        port.driver.as_deref().unwrap_or("unknown")
                    )
                })
                .unwrap_or_else(|| format!("configured RS485 port {declared} is not present")),
        });
    }

    if cfg.industrial.can_capture.enabled {
        if cfg.industrial.can_capture.interfaces.is_empty() {
            checks.push(DoctorCheck {
                name: "can.capture.config".into(),
                ok: false,
                detail: "capture is enabled but no interfaces are allowlisted".into(),
            });
        }
        for interface in &cfg.industrial.can_capture.interfaces {
            let found = inventory
                .industrial
                .can
                .iter()
                .find(|can| can.name == *interface);
            checks.push(DoctorCheck {
                name: format!("can.capture.{interface}"),
                ok: found.is_some(),
                detail: found
                    .map(|can| {
                        format!(
                            "read-only · state={} · max={} fps",
                            can.can_state.as_deref().unwrap_or(&can.operstate),
                            cfg.industrial.can_capture.max_frames_per_second.max(1)
                        )
                    })
                    .unwrap_or_else(|| "allowlisted interface is not present".into()),
            });
        }
    }

    for plugin in crate::plugins::statuses(cfg) {
        checks.push(DoctorCheck {
            name: format!("plugin.{}", plugin.manifest.name),
            ok: plugin.valid || !plugin.manifest.enabled,
            detail: if !plugin.manifest.enabled {
                "disabled".into()
            } else if plugin.valid {
                format!("{} ({})", plugin.manifest.command, plugin.manifest.version)
            } else {
                plugin.validation_errors.join("; ")
            },
        });
    }

    match profile::load(cfg) {
        Ok(profile) => {
            let ethernet = inventory
                .network
                .iter()
                .filter(|interface| interface.kind == "ethernet")
                .count();
            let values = [
                (
                    "profile.name",
                    cfg.device.profile == profile.name,
                    format!("expected {}, loaded {}", cfg.device.profile, profile.name),
                ),
                (
                    "profile.vendor",
                    inventory.device.vendor == profile.vendor,
                    format!(
                        "expected {}, found {}",
                        profile.vendor, inventory.device.vendor
                    ),
                ),
                (
                    "profile.arch",
                    inventory.system.arch == profile.arch,
                    format!("expected {}, found {}", profile.arch, inventory.system.arch),
                ),
                (
                    "profile.ethernet",
                    ethernet >= profile.minimum.ethernet,
                    format!("minimum {}, found {}", profile.minimum.ethernet, ethernet),
                ),
                (
                    "profile.gpio",
                    inventory.buses.gpio_chips.len() >= profile.minimum.gpio,
                    format!(
                        "minimum {}, found {}",
                        profile.minimum.gpio,
                        inventory.buses.gpio_chips.len()
                    ),
                ),
                (
                    "profile.i2c",
                    inventory.buses.i2c.len() >= profile.minimum.i2c,
                    format!(
                        "minimum {}, found {}",
                        profile.minimum.i2c,
                        inventory.buses.i2c.len()
                    ),
                ),
                (
                    "profile.spi",
                    inventory.buses.spi.len() >= profile.minimum.spi,
                    format!(
                        "minimum {}, found {}",
                        profile.minimum.spi,
                        inventory.buses.spi.len()
                    ),
                ),
                (
                    "profile.uart",
                    inventory.buses.uart.len() >= profile.minimum.uart,
                    format!(
                        "minimum {}, found {}",
                        profile.minimum.uart,
                        inventory.buses.uart.len()
                    ),
                ),
                (
                    "profile.can",
                    inventory.buses.can.len() >= profile.minimum.can,
                    format!(
                        "minimum {}, found {}",
                        profile.minimum.can,
                        inventory.buses.can.len()
                    ),
                ),
                (
                    "profile.usb",
                    inventory.usb.len() >= profile.minimum.usb,
                    format!(
                        "minimum {}, found {}",
                        profile.minimum.usb,
                        inventory.usb.len()
                    ),
                ),
                (
                    "profile.watchdog",
                    inventory.buses.watchdog.len() >= profile.minimum.watchdog,
                    format!(
                        "minimum {}, found {}",
                        profile.minimum.watchdog,
                        inventory.buses.watchdog.len()
                    ),
                ),
            ];
            for (name, ok, detail) in values {
                checks.push(DoctorCheck {
                    name: name.into(),
                    ok,
                    detail,
                });
            }
        }
        Err(error) => checks.push(DoctorCheck {
            name: "board-profile".into(),
            ok: false,
            detail: error.to_string(),
        }),
    }

    DoctorReport {
        ok: checks.iter().all(|check| check.ok),
        checks,
    }
}

pub fn collect_inventory_blocking(cfg: &Config) -> Inventory {
    let cfg = cfg.clone();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("hardware probe runtime");
        runtime.block_on(collect_inventory(&cfg))
    })
    .join()
    .expect("hardware probe thread")
}

pub fn doctor_blocking(cfg: &Config) -> DoctorReport {
    let cfg = cfg.clone();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("doctor runtime");
        runtime.block_on(doctor(&cfg))
    })
    .join()
    .expect("doctor thread")
}
