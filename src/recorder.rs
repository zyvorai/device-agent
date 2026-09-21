// SPDX-License-Identifier: Apache-2.0

//! Segmented, fsync'd operational journal.
//!
//! This is local evidence (hardware changes, health, connectivity). It is not
//! Nodra's application-message WAL.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{Config, RecorderConfig};
use crate::model::{AgentEvent, CanFrame};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Record {
    pub at_unix_ms: u64,
    pub kind: String,
    pub data: Value,
}

pub fn directory(cfg: &Config) -> PathBuf {
    if cfg.recorder.directory.is_empty() {
        PathBuf::from(&cfg.server.state_dir).join("recorder")
    } else {
        PathBuf::from(&cfg.recorder.directory)
    }
}

pub fn record_agent_event(cfg: &Config, event: &AgentEvent) {
    if !cfg.recorder.enabled {
        return;
    }
    let _ = append(
        &directory(cfg),
        &cfg.recorder,
        &Record {
            at_unix_ms: event.at_unix_ms,
            kind: event.kind.clone(),
            data: event.data.clone(),
        },
    );
}

pub fn record_can_frame(cfg: &Config, frame: &CanFrame) {
    if !cfg.recorder.enabled || !cfg.recorder.can_frames {
        return;
    }
    let data = if cfg.recorder.redact_can_payload {
        serde_json::json!({
            "interface": frame.interface,
            "can_id": frame.can_id,
            "dlc": frame.dlc,
            "payload": "redacted",
            "payload_bytes": frame.data.len(),
        })
    } else {
        serde_json::json!({
            "interface": frame.interface,
            "can_id": frame.can_id,
            "data_hex": frame.data_hex,
        })
    };
    let _ = append(
        &directory(cfg),
        &cfg.recorder,
        &Record {
            at_unix_ms: frame.captured_at_unix_ms,
            kind: "can.frame".into(),
            data,
        },
    );
}

pub fn append(dir: &Path, cfg: &RecorderConfig, record: &Record) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    rotate_if_needed(dir, cfg)?;
    let path = active_segment(dir)?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    serde_json::to_writer(&mut file, record)?;
    file.write_all(b"\n")?;
    file.flush()?;
    file.sync_all()?;
    enforce_cap(dir, cfg.max_bytes)?;
    Ok(())
}

pub fn query(dir: &Path, since_unix_ms: u64, until_unix_ms: u64) -> std::io::Result<Vec<Record>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("segment-") && name.ends_with(".jsonl"))
        })
        .collect();
    names.sort();
    let mut records = Vec::new();
    for path in names {
        let file = File::open(path)?;
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            let Ok(record) = serde_json::from_str::<Record>(&line) else {
                continue;
            };
            if record.at_unix_ms >= since_unix_ms && record.at_unix_ms <= until_unix_ms {
                records.push(record);
            }
        }
    }
    Ok(records)
}

fn active_segment(dir: &Path) -> std::io::Result<PathBuf> {
    let mut names = segment_paths(dir)?;
    if let Some(last) = names.pop() {
        return Ok(last);
    }
    Ok(dir.join("segment-000001.jsonl"))
}

fn rotate_if_needed(dir: &Path, cfg: &RecorderConfig) -> std::io::Result<()> {
    let path = active_segment(dir)?;
    if !path.exists() {
        return Ok(());
    }
    let len = fs::metadata(&path)?.len();
    if len < cfg.segment_bytes.max(1) {
        return Ok(());
    }
    let next = next_segment_name(dir)?;
    File::create(dir.join(next))?;
    Ok(())
}

fn next_segment_name(dir: &Path) -> std::io::Result<String> {
    let mut max = 0u32;
    for path in segment_paths(dir)? {
        if let Some(n) = segment_index(&path) {
            max = max.max(n);
        }
    }
    Ok(format!("segment-{:06}.jsonl", max + 1))
}

fn segment_index(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let number = name.strip_prefix("segment-")?.strip_suffix(".jsonl")?;
    number.parse().ok()
}

fn segment_paths(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut names: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| segment_index(path).is_some())
        .collect();
    names.sort();
    Ok(names)
}

fn enforce_cap(dir: &Path, max_bytes: u64) -> std::io::Result<()> {
    if max_bytes == 0 {
        return Ok(());
    }
    loop {
        let paths = segment_paths(dir)?;
        let total: u64 = paths
            .iter()
            .map(|path| fs::metadata(path).map(|m| m.len()).unwrap_or(0))
            .sum();
        if total <= max_bytes || paths.len() <= 1 {
            return Ok(());
        }
        fs::remove_file(&paths[0])?;
    }
}

pub fn parse_since(text: &str, now_unix_ms: u64) -> anyhow::Result<u64> {
    let text = text.trim();
    let (number, unit) = text
        .char_indices()
        .find_map(|(index, ch)| (!ch.is_ascii_digit()).then_some(index))
        .map(|index| text.split_at(index))
        .ok_or_else(|| anyhow::anyhow!("duration must look like 15m, 2h, or 30s"))?;
    let count: u64 = number
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid duration {text}"))?;
    let seconds = match unit {
        "s" => count,
        "m" => count.saturating_mul(60),
        "h" => count.saturating_mul(3600),
        "d" => count.saturating_mul(86400),
        _ => anyhow::bail!("duration unit must be s, m, h, or d"),
    };
    Ok(now_unix_ms.saturating_sub(seconds.saturating_mul(1000)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_round_trip_and_time_window() {
        let dir = std::env::temp_dir().join(format!("zyvor-recorder-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let cfg = RecorderConfig {
            enabled: true,
            directory: dir.display().to_string(),
            segment_bytes: 80,
            max_bytes: 10_000,
            can_frames: false,
            redact_can_payload: true,
        };
        append(
            &dir,
            &cfg,
            &Record {
                at_unix_ms: 1_000,
                kind: "inventory.changed".into(),
                data: serde_json::json!({"generation": 1}),
            },
        )
        .unwrap();
        append(
            &dir,
            &cfg,
            &Record {
                at_unix_ms: 5_000,
                kind: "nodra.disconnected".into(),
                data: serde_json::json!({"connected": false}),
            },
        )
        .unwrap();
        let window = query(&dir, 4_000, 9_000).unwrap();
        assert_eq!(window.len(), 1);
        assert_eq!(window[0].kind, "nodra.disconnected");
        let fixture = include_str!("../fixtures/recorder/segment-sample.jsonl");
        let parsed: Record = serde_json::from_str(fixture.lines().next().unwrap()).unwrap();
        assert_eq!(parsed.kind, "can.controller");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_relative_windows() {
        assert_eq!(
            parse_since("15m", 1_000_000).unwrap(),
            1_000_000 - 15 * 60 * 1000
        );
        assert_eq!(
            parse_since("2h", 10_000_000).unwrap(),
            10_000_000 - 2 * 3600 * 1000
        );
    }
}
