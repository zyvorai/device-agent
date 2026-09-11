// SPDX-License-Identifier: Apache-2.0

use crate::model::UsbDevice;
use std::{fs, path::Path};

fn read(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub fn collect() -> Vec<UsbDevice> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/bus/usb/devices") else {
        return out;
    };
    for entry in entries.flatten() {
        let base = entry.path();
        if !base.join("idVendor").exists() {
            continue;
        }
        out.push(UsbDevice {
            path: entry.file_name().to_string_lossy().to_string(),
            vendor_id: read(base.join("idVendor")),
            product_id: read(base.join("idProduct")),
            manufacturer: read(base.join("manufacturer")),
            product: read(base.join("product")),
        });
    }
    out
}
