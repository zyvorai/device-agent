// SPDX-License-Identifier: Apache-2.0

//! Plugin SDK v2 checks. Native execution stays in `plugins`. This module
//! validates the manifest contract and, when built with `--features wasm`,
//! runs a no-import Wasmtime module for untrusted plugins.

use anyhow::Context;
use serde_json::Value;

pub fn conformance(manifest: &Value) -> anyhow::Result<()> {
    let protocol = manifest
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or("v1");
    if protocol != "v2" {
        return Ok(());
    }
    for field in ["name", "version", "command"] {
        let value = manifest.get(field).and_then(Value::as_str).unwrap_or("");
        if value.is_empty() {
            anyhow::bail!("v2 manifest missing {field}");
        }
    }
    let capabilities = manifest
        .get("capabilities")
        .and_then(Value::as_array)
        .context("v2 manifest requires a capabilities array")?;
    if capabilities.is_empty() {
        anyhow::bail!("v2 manifest must declare at least one capability");
    }
    const KNOWN: &[&str] = &["i2c", "gpio", "network", "camera", "filesystem", "none"];
    for capability in capabilities {
        let name = capability.as_str().unwrap_or("");
        if !KNOWN.contains(&name) {
            anyhow::bail!("unknown capability {name}");
        }
    }
    let wants_hardware = capabilities
        .iter()
        .any(|cap| matches!(cap.as_str(), Some("i2c" | "gpio" | "camera")));
    let runtime = manifest
        .get("runtime")
        .and_then(Value::as_str)
        .unwrap_or("native");
    if wants_hardware && runtime == "wasi" {
        anyhow::bail!("hardware capabilities cannot use the WASI runtime");
    }
    Ok(())
}

pub fn run_wasi_plugin(bytes: &[u8]) -> anyhow::Result<i32> {
    if bytes.len() < 4 || &bytes[..4] != b"\0asm" {
        anyhow::bail!("not a Wasm module");
    }
    #[cfg(feature = "wasm")]
    {
        return wasm_exec(bytes);
    }
    #[cfg(not(feature = "wasm"))]
    {
        let _ = bytes;
        anyhow::bail!("WASI plugins require a binary built with --features wasm");
    }
}

#[cfg(feature = "wasm")]
fn wasm_exec(bytes: &[u8]) -> anyhow::Result<i32> {
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, bytes).context("compiling wasm plugin")?;
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[])
        .context("instantiating wasm plugin (no host imports)")?;
    let function = instance
        .get_typed_func::<(), i32>(&mut store, "zyvor_plugin_ok")
        .context("wasm plugin must export zyvor_plugin_ok() -> i32")?;
    function
        .call(&mut store, ())
        .context("calling zyvor_plugin_ok")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_requires_a_capability_and_rejects_hardware_on_wasi() {
        let ok = serde_json::json!({
            "protocol": "v2",
            "name": "temp",
            "version": "1",
            "command": "/usr/bin/true",
            "capabilities": ["none"],
            "runtime": "wasi"
        });
        conformance(&ok).unwrap();
        let bad = serde_json::json!({
            "protocol": "v2",
            "name": "temp",
            "version": "1",
            "command": "/usr/bin/true",
            "capabilities": ["i2c"],
            "runtime": "wasi"
        });
        assert!(conformance(&bad).is_err());
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/plugins/manifest-v2.json")).unwrap();
        conformance(&fixture).unwrap();
    }

    #[test]
    fn non_wasm_bytes_are_rejected() {
        assert!(run_wasi_plugin(b"not wasm").is_err());
    }
}
