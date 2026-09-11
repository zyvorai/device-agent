// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
    sync::Arc,
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use serde::{Deserialize, Serialize};
use tokio::{
    process::Command,
    time::{interval, timeout, Instant, MissedTickBehavior},
};
use tracing::{info, warn};

use crate::{
    config::Config,
    model::{SensorReading, SensorSample},
    state::{now_unix_ms, AppState},
};

fn default_enabled() -> bool {
    true
}

fn default_publish() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub sensor_id: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub poll_interval_seconds: Option<u64>,
    #[serde(default = "default_publish")]
    pub publish_to_nodra: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStatus {
    #[serde(flatten)]
    pub manifest: PluginManifest,
    pub valid: bool,
    pub validation_errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ProtocolReading {
    name: Option<String>,
    kind: Option<String>,
    value: f64,
    unit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProtocolOutput {
    sensor: Option<String>,
    value: Option<f64>,
    unit: Option<String>,
    kind: Option<String>,
    quality: Option<String>,
    labels: Option<BTreeMap<String, String>>,
    readings: Option<Vec<ProtocolReading>>,
}

pub fn discover(cfg: &Config) -> Vec<PluginManifest> {
    let Ok(entries) = fs::read_dir(&cfg.plugins.directory) else {
        return vec![];
    };
    let mut out: Vec<PluginManifest> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|v| v.to_str()) != Some("json") {
            continue;
        }
        if let Ok(raw) = fs::read_to_string(path) {
            if let Ok(manifest) = serde_json::from_str(&raw) {
                out.push(manifest);
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn statuses(cfg: &Config) -> Vec<PluginStatus> {
    discover(cfg)
        .into_iter()
        .map(|manifest| {
            let validation_errors = validate(cfg, &manifest);
            PluginStatus {
                valid: validation_errors.is_empty(),
                manifest,
                validation_errors,
            }
        })
        .collect()
}

pub fn validate(cfg: &Config, manifest: &PluginManifest) -> Vec<String> {
    let mut errors = Vec::new();
    let command = Path::new(&manifest.command);

    if cfg.plugins.require_absolute_command && !command.is_absolute() {
        errors.push("plugin command must be an absolute path".into());
    }

    match fs::metadata(command) {
        Ok(metadata) => {
            if !metadata.is_file() {
                errors.push("plugin command is not a regular file".into());
            }
            #[cfg(unix)]
            {
                let mode = metadata.permissions().mode();
                if mode & 0o111 == 0 {
                    errors.push("plugin command is not executable".into());
                }
                if cfg.plugins.reject_world_writable && mode & 0o002 != 0 {
                    errors.push("plugin command is world-writable".into());
                }
            }
        }
        Err(error) => errors.push(format!("plugin command unavailable: {error}")),
    }

    if manifest.name.trim().is_empty() {
        errors.push("plugin name is empty".into());
    }
    if manifest.version.trim().is_empty() {
        errors.push("plugin version is empty".into());
    }
    errors
}

pub async fn sample(cfg: &Config, manifest: &PluginManifest) -> SensorSample {
    let sensor_id = manifest
        .sensor_id
        .clone()
        .unwrap_or_else(|| manifest.name.clone());
    let validation_errors = validate(cfg, manifest);
    if !validation_errors.is_empty() {
        return failed_sample(
            &sensor_id,
            &manifest.name,
            manifest.publish_to_nodra,
            format!("plugin rejected: {}", validation_errors.join("; ")),
        );
    }

    let mut cmd = Command::new(&manifest.command);
    cmd.args(&manifest.args)
        .env("ZYVOR_PLUGIN_PROTOCOL", "v1")
        .kill_on_drop(true);

    let result = timeout(
        Duration::from_secs(cfg.plugins.timeout_seconds.max(1)),
        cmd.output(),
    )
    .await;
    match result {
        Ok(Ok(output)) if output.status.success() => {
            if output.stdout.len() > cfg.plugins.max_output_bytes {
                return failed_sample(
                    &sensor_id,
                    &manifest.name,
                    manifest.publish_to_nodra,
                    format!(
                        "plugin output exceeded {} bytes",
                        cfg.plugins.max_output_bytes
                    ),
                );
            }
            match serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                Ok(raw) => normalize_sample(manifest, raw),
                Err(error) => failed_sample(
                    &sensor_id,
                    &manifest.name,
                    manifest.publish_to_nodra,
                    format!("plugin returned invalid JSON: {error}"),
                ),
            }
        }
        Ok(Ok(output)) => failed_sample(
            &sensor_id,
            &manifest.name,
            manifest.publish_to_nodra,
            format!(
                "plugin exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ),
        Ok(Err(error)) => failed_sample(
            &sensor_id,
            &manifest.name,
            manifest.publish_to_nodra,
            error.to_string(),
        ),
        Err(_) => failed_sample(
            &sensor_id,
            &manifest.name,
            manifest.publish_to_nodra,
            "plugin timeout".into(),
        ),
    }
}

pub async fn scheduler_loop(state: Arc<AppState>) -> anyhow::Result<()> {
    let mut ticker = interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut last_sampled: HashMap<String, Instant> = HashMap::new();

    loop {
        ticker.tick().await;
        for manifest in discover(&state.config) {
            if !manifest.enabled {
                continue;
            }
            let cadence = Duration::from_secs(
                manifest
                    .poll_interval_seconds
                    .unwrap_or(state.config.plugins.sample_interval_seconds)
                    .max(1),
            );
            if let Some(last) = last_sampled.get(&manifest.name) {
                if last.elapsed() < cadence {
                    continue;
                }
            }

            let sample = sample(&state.config, &manifest).await;
            if sample.ok {
                info!(plugin = %manifest.name, sensor = %sample.sensor_id, "sensor sample collected");
            } else {
                warn!(plugin = %manifest.name, error = ?sample.error, "sensor sample failed");
            }
            state.record_sample(sample).await;
            last_sampled.insert(manifest.name.clone(), Instant::now());
        }
    }
}

fn normalize_sample(manifest: &PluginManifest, raw: serde_json::Value) -> SensorSample {
    let parsed = serde_json::from_value::<ProtocolOutput>(raw.clone());
    let Ok(parsed) = parsed else {
        return SensorSample {
            sensor_id: manifest
                .sensor_id
                .clone()
                .unwrap_or_else(|| manifest.name.clone()),
            plugin: manifest.name.clone(),
            collected_at_unix_ms: now_unix_ms(),
            ok: true,
            quality: "unknown".into(),
            publish_to_nodra: manifest.publish_to_nodra,
            readings: vec![],
            labels: BTreeMap::new(),
            error: None,
            raw,
        };
    };

    let sensor_id = manifest
        .sensor_id
        .clone()
        .or(parsed.sensor.clone())
        .unwrap_or_else(|| manifest.name.clone());
    let mut readings = parsed
        .readings
        .unwrap_or_default()
        .into_iter()
        .map(|reading| SensorReading {
            name: reading.name.unwrap_or_else(|| "value".into()),
            kind: reading.kind.unwrap_or_default(),
            value: reading.value,
            unit: reading.unit.unwrap_or_default(),
        })
        .collect::<Vec<_>>();

    if readings.is_empty() && parsed.value.is_some() {
        let value = parsed.value.unwrap_or_default();
        readings.push(SensorReading {
            name: parsed.sensor.clone().unwrap_or_else(|| "value".into()),
            kind: parsed
                .kind
                .or_else(|| manifest.capabilities.first().cloned())
                .unwrap_or_default(),
            value,
            unit: parsed.unit.unwrap_or_default(),
        });
    }

    SensorSample {
        sensor_id,
        plugin: manifest.name.clone(),
        collected_at_unix_ms: now_unix_ms(),
        ok: true,
        quality: parsed.quality.unwrap_or_else(|| "good".into()),
        publish_to_nodra: manifest.publish_to_nodra,
        readings,
        labels: parsed.labels.unwrap_or_default(),
        error: None,
        raw,
    }
}

fn failed_sample(
    sensor_id: &str,
    plugin: &str,
    publish_to_nodra: bool,
    error: String,
) -> SensorSample {
    SensorSample {
        sensor_id: sensor_id.to_string(),
        plugin: plugin.to_string(),
        collected_at_unix_ms: now_unix_ms(),
        ok: false,
        quality: "error".into(),
        publish_to_nodra,
        readings: vec![],
        labels: BTreeMap::new(),
        error: Some(error),
        raw: serde_json::Value::Null,
    }
}
