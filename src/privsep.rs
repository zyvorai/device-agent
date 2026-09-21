// SPDX-License-Identifier: Apache-2.0

//! Privilege-separated bus helper.
//!
//! With `[privsep].enabled = false` (the default) the API process opens buses
//! directly and this module is unused. With `enabled = true` and
//! `--features privsep`, `serve` spawns `bus-helper` and inventory/doctor/CAN/
//! camera operations go over an authenticated Unix socket. The helper allowlists
//! opcodes: there is no shell and no arbitrary exec.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command};
use std::time::Duration;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::{Config, PrivsepConfig};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PrivsepStatus {
    pub enabled: bool,
    pub feature_compiled: bool,
    pub helper_running: bool,
    pub helper_path: String,
    pub socket_path: String,
    pub note: &'static str,
}

pub const FEATURE_COMPILED: bool = cfg!(feature = "privsep");

/// True when the API process may open device nodes itself.
pub fn direct_bus_access(config: &PrivsepConfig) -> bool {
    !(config.enabled && FEATURE_COMPILED)
}

pub fn status(config: &PrivsepConfig) -> PrivsepStatus {
    status_with_runtime(config, false)
}

pub fn status_with_runtime(config: &PrivsepConfig, helper_running: bool) -> PrivsepStatus {
    let note = if !config.enabled {
        "privsep disabled (default) — daemon uses direct bus access as today"
    } else if !FEATURE_COMPILED {
        "privsep.enabled set but this binary was built without --features privsep — daemon bus access unchanged"
    } else if helper_running {
        "bus helper is running; API process does not open device nodes"
    } else {
        "privsep compiled; helper starts on serve and owns bus access"
    };
    PrivsepStatus {
        enabled: config.enabled,
        feature_compiled: FEATURE_COMPILED,
        helper_running,
        helper_path: config.helper_path.clone(),
        socket_path: config.socket_path.clone(),
        note,
    }
}

pub fn warn_if_requested(config: &PrivsepConfig) {
    if !config.enabled || FEATURE_COMPILED {
        return;
    }
    let snap = status(config);
    tracing::warn!(
        helper_path = %snap.helper_path,
        socket_path = %snap.socket_path,
        feature_compiled = snap.feature_compiled,
        "{}",
        snap.note
    );
}

pub fn peer_allowed(config: &PrivsepConfig, uid: u32, gid: u32) -> bool {
    if config.allow_uids.is_empty() && config.allow_gids.is_empty() {
        return uid == current_uid();
    }
    config.allow_uids.contains(&uid) || config.allow_gids.contains(&gid)
}

fn current_uid() -> u32 {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    op: String,
    #[serde(default)]
    interface: String,
    #[serde(default)]
    camera_id: String,
}

/// Allowlisted operations. Unknown opcodes fail before any device open.
pub fn dispatch_opcode(cfg: &Config, op: &str) -> Result<Value, String> {
    let request = RpcRequest {
        op: op.to_string(),
        interface: String::new(),
        camera_id: String::new(),
    };
    dispatch(cfg, &request)
}

fn dispatch(cfg: &Config, request: &RpcRequest) -> Result<Value, String> {
    match request.op.as_str() {
        "inventory" => {
            let inventory = crate::hardware::collect_inventory_blocking(cfg);
            serde_json::to_value(inventory).map_err(|error| error.to_string())
        }
        "doctor" => {
            let report = crate::hardware::doctor_blocking(cfg);
            serde_json::to_value(report).map_err(|error| error.to_string())
        }
        "can-rx" => can_rx(cfg, &request.interface),
        "camera-read" => camera_read(cfg, &request.camera_id),
        _ => Err(format!("unknown opcode {}", request.op)),
    }
}

fn can_rx(cfg: &Config, interface: &str) -> Result<Value, String> {
    if interface.is_empty() {
        return Err("can-rx requires an interface".into());
    }
    let allowed = &cfg.industrial.can_capture.interfaces;
    if !allowed.iter().any(|name| name == interface) {
        return Err(format!("CAN interface {interface} is not allowlisted"));
    }
    // Receive-only snapshot. Transmit is not an opcode.
    match crate::can_capture::rx_once(interface) {
        Ok(frame) => Ok(json!({ "interface": interface, "frame": frame, "transmit": false })),
        Err(error) => Ok(json!({
            "interface": interface,
            "frame": null,
            "transmit": false,
            "error": error
        })),
    }
}

fn camera_read(cfg: &Config, camera_id: &str) -> Result<Value, String> {
    if camera_id.is_empty() {
        return Err("camera-read requires a camera id".into());
    }
    let Some(device) = cfg
        .camera
        .devices
        .iter()
        .find(|device| device.id == camera_id)
    else {
        return Err(format!("camera {camera_id} is not allowlisted"));
    };
    match open_video_readonly(&device.path) {
        Ok(bytes) => Ok(json!({
            "id": camera_id,
            "path": device.path,
            "bytes": bytes,
            "opened": true
        })),
        Err(error) => Ok(json!({
            "id": camera_id,
            "path": device.path,
            "opened": false,
            "error": error
        })),
    }
}

fn open_video_readonly(path: &str) -> Result<u64, String> {
    if !path.starts_with("/dev/video") {
        return Err("camera path is not a /dev/video node".into());
    }
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let len = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    Ok(len)
}

/// Blocking helper server. One JSON object per line, one JSON object back.
pub fn serve_helper(cfg: &Config) -> anyhow::Result<()> {
    #[cfg(not(unix))]
    {
        let _ = cfg;
        anyhow::bail!("bus-helper requires a Unix domain socket");
    }
    #[cfg(unix)]
    {
        serve_helper_unix(cfg, None)
    }
}

#[cfg(unix)]
pub fn serve_helper_unix(cfg: &Config, max_connections: Option<usize>) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    let path = Path::new(&cfg.privsep.socket_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(path);
    let listener =
        UnixListener::bind(path).with_context(|| format!("binding {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o660)).ok();
    for (handled, incoming) in listener.incoming().enumerate() {
        let stream = incoming?;
        handle_stream(cfg, stream);
        if max_connections.is_some_and(|limit| handled + 1 >= limit) {
            break;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn handle_stream(cfg: &Config, stream: std::os::unix::net::UnixStream) {
    let peer = peer_ids(&stream);
    let mut writer = stream.try_clone().ok();
    let (uid, gid) = match peer {
        Ok(ids) => ids,
        Err(error) => {
            let _ = write_line(
                writer.as_mut(),
                &json!({"ok": false, "error": format!("peer credentials: {error}")}),
            );
            return;
        }
    };
    if !peer_allowed(&cfg.privsep, uid, gid) {
        let _ = write_line(
            writer.as_mut(),
            &json!({"ok": false, "error": "peer uid not allowed"}),
        );
        return;
    }
    let reader = BufReader::new(stream);
    for line in reader.lines().take(1) {
        let Ok(line) = line else { break };
        let response = match serde_json::from_str::<RpcRequest>(&line) {
            Ok(request) => match dispatch(cfg, &request) {
                Ok(value) => json!({"ok": true, "result": value}),
                Err(error) => json!({"ok": false, "error": error}),
            },
            Err(error) => json!({"ok": false, "error": format!("invalid request: {error}")}),
        };
        let _ = write_line(writer.as_mut(), &response);
    }
}

#[cfg(unix)]
fn write_line(stream: Option<&mut impl Write>, value: &Value) -> std::io::Result<()> {
    let Some(stream) = stream else {
        return Ok(());
    };
    serde_json::to_writer(&mut *stream, value)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

#[cfg(all(unix, target_os = "linux"))]
fn peer_ids(stream: &std::os::unix::net::UnixStream) -> std::io::Result<(u32, u32)> {
    let cred = nix::sys::socket::getsockopt(stream, nix::sys::socket::sockopt::PeerCredentials)
        .map_err(|error| std::io::Error::other(error))?;
    Ok((cred.uid() as u32, cred.gid() as u32))
}

#[cfg(all(unix, not(target_os = "linux")))]
fn peer_ids(stream: &std::os::unix::net::UnixStream) -> std::io::Result<(u32, u32)> {
    use std::os::unix::io::AsRawFd;
    let mut uid = 0u32;
    let mut gid = 0u32;
    let rc = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok((uid, gid))
}

pub struct HelperProcess {
    child: Child,
}

impl Drop for HelperProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn spawn_helper_process(cfg: &Config, config_path: &Path) -> anyhow::Result<HelperProcess> {
    let exe = std::env::current_exe().context("locating the device-agent binary")?;
    let program = if Path::new(&cfg.privsep.helper_path).is_file() {
        cfg.privsep.helper_path.clone()
    } else {
        exe.display().to_string()
    };
    let mut child = Command::new(&program)
        .arg("--config")
        .arg(config_path)
        .arg("bus-helper")
        .spawn()
        .with_context(|| format!("spawning bus helper {program}"))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if Path::new(&cfg.privsep.socket_path).exists() {
            return Ok(HelperProcess { child });
        }
        if let Some(status) = child.try_wait().ok().flatten() {
            anyhow::bail!("bus helper exited before binding the socket ({status})");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    anyhow::bail!("bus helper did not create {}", cfg.privsep.socket_path);
}

pub async fn fetch_inventory(cfg: &Config) -> anyhow::Result<crate::model::Inventory> {
    let value = rpc(cfg, json!({"op": "inventory"})).await?;
    serde_json::from_value(value).context("decoding helper inventory")
}

pub async fn fetch_doctor(cfg: &Config) -> anyhow::Result<crate::model::DoctorReport> {
    let value = rpc(cfg, json!({"op": "doctor"})).await?;
    serde_json::from_value(value).context("decoding helper doctor report")
}

async fn rpc(cfg: &Config, request: Value) -> anyhow::Result<Value> {
    let socket = cfg.privsep.socket_path.clone();
    let response = tokio::task::spawn_blocking(move || rpc_blocking(&socket, &request))
        .await
        .context("joining bus-helper call")??;
    Ok(response)
}

fn rpc_blocking(socket: &str, request: &Value) -> anyhow::Result<Value> {
    #[cfg(not(unix))]
    {
        let _ = (socket, request);
        anyhow::bail!("bus helper RPC requires Unix");
    }
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;
        let mut stream =
            UnixStream::connect(socket).with_context(|| format!("connecting to {socket}"))?;
        serde_json::to_writer(&mut stream, request)?;
        stream.write_all(b"\n")?;
        stream.flush()?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let parsed: Value = serde_json::from_str(&line).context("parsing helper response")?;
        if parsed.get("ok").and_then(Value::as_bool) != Some(true) {
            let error = parsed
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("helper error");
            anyhow::bail!("{error}");
        }
        Ok(parsed.get("result").cloned().unwrap_or(Value::Null))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_status_is_inert() {
        let snap = status(&PrivsepConfig::default());
        assert!(!snap.enabled);
        assert!(!snap.helper_running);
        assert_eq!(snap.feature_compiled, FEATURE_COMPILED);
        assert!(snap.note.contains("disabled"));
        assert!(direct_bus_access(&PrivsepConfig::default()));
    }

    #[test]
    fn enabled_without_a_running_helper_is_not_marked_running() {
        let config = PrivsepConfig {
            enabled: true,
            helper_path: "/usr/lib/zyvor-device-agent/bus-helper".into(),
            socket_path: "/run/zyvor-device-agent/bus.sock".into(),
            allow_uids: Vec::new(),
            allow_gids: Vec::new(),
        };
        let snap = status(&config);
        assert!(snap.enabled);
        assert!(!snap.helper_running);
    }

    #[test]
    fn unknown_opcode_is_rejected() {
        let error = dispatch_opcode(&Config::default(), "shell").unwrap_err();
        assert!(error.contains("unknown opcode"));
        assert!(error.contains("shell"));
    }

    #[test]
    fn peer_outside_the_allowlist_is_rejected() {
        let config = PrivsepConfig {
            enabled: true,
            allow_uids: vec![1],
            allow_gids: Vec::new(),
            ..PrivsepConfig::default()
        };
        assert!(!peer_allowed(&config, 1000, 1000));
        assert!(peer_allowed(&config, 1, 1000));
    }

    #[cfg(unix)]
    #[test]
    fn helper_rejects_peer_and_unknown_opcode_over_the_socket() {
        let dir =
            std::env::temp_dir().join(format!("zyvor-privsep-{}-{}", std::process::id(), unique()));
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("bus.sock");
        let mut cfg = Config::default();
        cfg.privsep.enabled = true;
        cfg.privsep.socket_path = socket.display().to_string();
        cfg.privsep.allow_uids = vec![1];
        let cfg_reject = cfg.clone();
        let server = std::thread::spawn(move || {
            serve_helper_unix(&cfg_reject, Some(1)).unwrap();
        });
        wait_for_socket(&socket);
        let rejected = call(&socket, r#"{"op":"inventory"}"#);
        assert!(rejected.contains("peer uid not allowed"), "{rejected}");
        server.join().unwrap();
        let _ = std::fs::remove_file(&socket);

        cfg.privsep.allow_uids = vec![current_uid()];
        let cfg_opcode = cfg.clone();
        let server = std::thread::spawn(move || {
            serve_helper_unix(&cfg_opcode, Some(1)).unwrap();
        });
        wait_for_socket(&socket);
        let unknown = call(&socket, r#"{"op":"shell"}"#);
        assert!(unknown.contains("unknown opcode"), "{unknown}");
        server.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    fn unique() -> u64 {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    #[cfg(unix)]
    fn wait_for_socket(path: &Path) {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if path.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("socket {} was not created", path.display());
    }

    #[cfg(unix)]
    fn call(path: &Path, line: &str) -> String {
        use std::os::unix::net::UnixStream;
        let mut stream = UnixStream::connect(path).unwrap();
        stream.write_all(line.as_bytes()).unwrap();
        stream.write_all(b"\n").unwrap();
        stream.flush().unwrap();
        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_line(&mut response).unwrap();
        response
    }
}
