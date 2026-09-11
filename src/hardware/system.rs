// SPDX-License-Identifier: Apache-2.0

use crate::model::SystemInfo;
use std::{fs, process::Command, thread};

fn read_trim(path: &str) -> String {
    fs::read_to_string(path)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn os_pretty_name() -> String {
    let raw = read_trim("/etc/os-release");
    raw.lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').to_string())
        .unwrap_or_else(|| "Linux".into())
}

fn cpu_model() -> String {
    let raw = read_trim("/proc/cpuinfo");
    for key in ["model name", "Hardware", "Processor"] {
        if let Some(line) = raw.lines().find(|l| l.starts_with(key)) {
            if let Some((_, value)) = line.split_once(':') {
                return value.trim().to_string();
            }
        }
    }
    std::env::consts::ARCH.to_string()
}

fn memory_bytes() -> u64 {
    let raw = read_trim("/proc/meminfo");
    raw.lines()
        .find_map(|line| {
            let mut p = line.split_whitespace();
            if p.next()? == "MemTotal:" {
                p.next()?.parse::<u64>().ok().map(|kb| kb * 1024)
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn uptime_seconds() -> u64 {
    read_trim("/proc/uptime")
        .split_whitespace()
        .next()
        .and_then(|v| v.split('.').next())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

fn storage_bytes() -> Option<u64> {
    // `df -kP` is available on GNU coreutils and BusyBox-style edge images.
    let output = Command::new("df").args(["-kP", "/"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let fields: Vec<&str> = text.lines().nth(1)?.split_whitespace().collect();
    fields.get(1)?.parse::<u64>().ok().map(|kib| kib * 1024)
}

pub fn collect() -> SystemInfo {
    SystemInfo {
        arch: std::env::consts::ARCH.into(),
        kernel: read_trim("/proc/sys/kernel/osrelease"),
        os: os_pretty_name(),
        cpu_model: cpu_model(),
        cpu_cores: thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
        memory_bytes: memory_bytes(),
        storage_bytes: storage_bytes(),
        uptime_seconds: uptime_seconds(),
    }
}
