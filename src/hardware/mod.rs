// SPDX-License-Identifier: Apache-2.0

mod buses;
mod identity;
mod network;
mod system;
mod thermal;
mod usb;

use crate::{
    config::Config,
    model::{DoctorCheck, DoctorReport, Inventory},
    profile,
};

pub async fn collect_inventory(cfg: &Config) -> Inventory {
    let device = identity::collect(cfg);
    let system = system::collect();
    let network = network::collect();
    let buses = buses::collect();
    let usb = usb::collect();
    let thermal = thermal::collect();

    let mut capabilities = vec![
        "hardware-inventory".into(),
        "system-health".into(),
        "sensor-plugin-api".into(),
        "sensor-scheduler".into(),
        "hardware-events".into(),
        "fleet-inventory-projection".into(),
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
    }
    if !buses.can.is_empty() {
        capabilities.push("can".into());
    }
    if !buses.watchdog.is_empty() {
        capabilities.push("watchdog".into());
    }

    Inventory {
        device,
        system,
        network,
        buses,
        usb,
        thermal,
        capabilities,
    }
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
        ok: inventory.network.iter().any(|n| n.name != "lo"),
        detail: inventory
            .network
            .iter()
            .filter(|n| n.name != "lo")
            .map(|n| format!("{}:{}", n.name, n.operstate))
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
        Ok(p) => {
            let ethernet = inventory
                .network
                .iter()
                .filter(|n| n.kind == "ethernet")
                .count();
            let values = [
                (
                    "profile.name",
                    cfg.device.profile == p.name,
                    format!("expected {}, loaded {}", cfg.device.profile, p.name),
                ),
                (
                    "profile.vendor",
                    inventory.device.vendor == p.vendor,
                    format!("expected {}, found {}", p.vendor, inventory.device.vendor),
                ),
                (
                    "profile.arch",
                    inventory.system.arch == p.arch,
                    format!("expected {}, found {}", p.arch, inventory.system.arch),
                ),
                (
                    "profile.ethernet",
                    ethernet >= p.minimum.ethernet,
                    format!("minimum {}, found {}", p.minimum.ethernet, ethernet),
                ),
                (
                    "profile.gpio",
                    inventory.buses.gpio_chips.len() >= p.minimum.gpio,
                    format!(
                        "minimum {}, found {}",
                        p.minimum.gpio,
                        inventory.buses.gpio_chips.len()
                    ),
                ),
                (
                    "profile.i2c",
                    inventory.buses.i2c.len() >= p.minimum.i2c,
                    format!(
                        "minimum {}, found {}",
                        p.minimum.i2c,
                        inventory.buses.i2c.len()
                    ),
                ),
                (
                    "profile.spi",
                    inventory.buses.spi.len() >= p.minimum.spi,
                    format!(
                        "minimum {}, found {}",
                        p.minimum.spi,
                        inventory.buses.spi.len()
                    ),
                ),
                (
                    "profile.uart",
                    inventory.buses.uart.len() >= p.minimum.uart,
                    format!(
                        "minimum {}, found {}",
                        p.minimum.uart,
                        inventory.buses.uart.len()
                    ),
                ),
                (
                    "profile.can",
                    inventory.buses.can.len() >= p.minimum.can,
                    format!(
                        "minimum {}, found {}",
                        p.minimum.can,
                        inventory.buses.can.len()
                    ),
                ),
                (
                    "profile.usb",
                    inventory.usb.len() >= p.minimum.usb,
                    format!("minimum {}, found {}", p.minimum.usb, inventory.usb.len()),
                ),
                (
                    "profile.watchdog",
                    inventory.buses.watchdog.len() >= p.minimum.watchdog,
                    format!(
                        "minimum {}, found {}",
                        p.minimum.watchdog,
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
        ok: checks.iter().all(|c| c.ok),
        checks,
    }
}
