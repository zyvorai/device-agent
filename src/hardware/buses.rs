// SPDX-License-Identifier: Apache-2.0

use std::fs;
use crate::model::BusInventory;

fn glob_prefix(dir: &str, prefix: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else { return vec![]; };
    let mut out: Vec<_> = entries.flatten().filter_map(|e| {
        let name = e.file_name().to_string_lossy().to_string();
        name.starts_with(prefix).then_some(name)
    }).collect();
    out.sort(); out
}

pub fn collect() -> BusInventory {
    let mut can = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/class/net") {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("can") || name.starts_with("vcan") { can.push(name); }
        }
    }
    BusInventory {
        gpio_chips: glob_prefix("/dev", "gpiochip"),
        i2c: glob_prefix("/dev", "i2c-"),
        spi: glob_prefix("/dev", "spidev"),
        uart: ["ttyS", "ttyAMA", "ttyUSB", "ttyACM"].into_iter().flat_map(|p| glob_prefix("/dev", p)).collect(),
        can,
        watchdog: glob_prefix("/dev", "watchdog"),
    }
}
