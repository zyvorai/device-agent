// SPDX-License-Identifier: Apache-2.0

use std::{fs, path::PathBuf, time::Duration};
use serde::{Deserialize, Serialize};
use tokio::{process::Command, time::timeout};

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResult {
    pub name: String,
    pub ok: bool,
    pub output: serde_json::Value,
}

pub fn discover(cfg: &Config) -> Vec<PluginManifest> {
    let Ok(entries) = fs::read_dir(&cfg.plugins.directory) else { return vec![]; };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|v| v.to_str()) != Some("json") { continue; }
        if let Ok(raw) = fs::read_to_string(path) {
            if let Ok(manifest) = serde_json::from_str(&raw) { out.push(manifest); }
        }
    }
    out.sort_by(|a,b| a.name.cmp(&b.name)); out
}

pub async fn sample(cfg: &Config, manifest: &PluginManifest) -> PluginResult {
    let mut cmd = Command::new(&manifest.command);
    cmd.args(&manifest.args).env("ZYVOR_PLUGIN_PROTOCOL", "v1");
    let result = timeout(Duration::from_secs(cfg.plugins.timeout_seconds), cmd.output()).await;
    match result {
        Ok(Ok(output)) if output.status.success() => {
            let parsed = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| serde_json::json!({"raw": String::from_utf8_lossy(&output.stdout)}));
            PluginResult { name: manifest.name.clone(), ok: true, output: parsed }
        }
        Ok(Ok(output)) => PluginResult { name: manifest.name.clone(), ok: false, output: serde_json::json!({"stderr": String::from_utf8_lossy(&output.stderr)}) },
        Ok(Err(err)) => PluginResult { name: manifest.name.clone(), ok: false, output: serde_json::json!({"error": err.to_string()}) },
        Err(_) => PluginResult { name: manifest.name.clone(), ok: false, output: serde_json::json!({"error": "plugin timeout"}) },
    }
}
