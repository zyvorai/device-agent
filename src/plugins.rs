// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
    sync::Arc,
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use serde::{Deserialize, Serialize};
use tokio::{
    process::Command,
    time::{interval, timeout, Instant, MissedTickBehavior},
};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::{
    config::{Config, PluginConfig},
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
                if !cfg.plugins.allowed_owners.is_empty() {
                    let owner_uid = metadata.uid();
                    let allowed = cfg
                        .plugins
                        .allowed_owners
                        .iter()
                        .any(|entry| resolve_uid(entry) == Some(owner_uid));
                    if !allowed {
                        errors.push(format!(
                            "plugin command owner uid {owner_uid} is not in allowed_owners"
                        ));
                    }
                }
            }
        }
        Err(error) => errors.push(format!("plugin command unavailable: {error}")),
    }

    if !cfg.plugins.allowed_directories.is_empty() {
        // Canonicalize rather than string-prefix-match the manifest's raw path, so a
        // `../` traversal or a symlink pointing outside the allowlist can't sneak past.
        let within = fs::canonicalize(command).is_ok_and(|canonical| {
            cfg.plugins
                .allowed_directories
                .iter()
                .any(|dir| canonical.starts_with(dir))
        });
        if !within {
            errors.push("plugin command is outside allowed_directories".into());
        }
    }

    if manifest.name.trim().is_empty() {
        errors.push("plugin name is empty".into());
    }
    if manifest.version.trim().is_empty() {
        errors.push("plugin version is empty".into());
    }
    errors
}

/// Resolves a config entry (numeric uid, or a username to look up) to a uid.
#[cfg(unix)]
fn resolve_uid(value: &str) -> Option<u32> {
    if let Ok(uid) = value.parse::<u32>() {
        return Some(uid);
    }
    nix::unistd::User::from_name(value)
        .ok()
        .flatten()
        .map(|user| user.uid.as_raw())
}

/// Whether `apply_process_hardening` has anything to do for this config — pulled out
/// as a pure, platform-independent predicate so it's directly unit-testable without
/// needing to inspect a `Command`'s internal `pre_exec` state. Only ever called from
/// the `#[cfg(target_os = "linux")]` half of `apply_process_hardening` (privilege drop
/// and rlimits are Linux-only), so it has no real caller on other platforms outside
/// this file's own unix test module.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn hardening_is_needed(cfg: &PluginConfig) -> bool {
    cfg.run_as_uid.is_some()
        || cfg.run_as_gid.is_some()
        || cfg.max_memory_bytes.is_some()
        || cfg.max_cpu_seconds.is_some()
        || cfg.max_processes.is_some()
        || cfg.seccomp_enabled
}

/// Drops the plugin subprocess to `plugins.run_as_uid`/`run_as_gid` when configured
/// (both `None` by default — inherits the daemon's own identity, today's behavior),
/// and/or applies `plugins.max_memory_bytes`/`max_cpu_seconds`/`max_processes` resource
/// limits (all `None` by default — unlimited, today's behavior). Also clears
/// supplementary groups when dropping identity, so the child doesn't inherit the
/// daemon's full group membership (relevant since the daemon itself runs as root).
#[cfg(target_os = "linux")]
fn apply_process_hardening(cmd: &mut Command, cfg: &Config) {
    if !hardening_is_needed(&cfg.plugins) {
        return;
    }
    let uid = cfg.plugins.run_as_uid;
    let gid = cfg.plugins.run_as_gid;
    let max_memory_bytes = cfg.plugins.max_memory_bytes;
    let max_cpu_seconds = cfg.plugins.max_cpu_seconds;
    let max_processes = cfg.plugins.max_processes;
    let seccomp_enabled = cfg.plugins.seccomp_enabled;
    let drop_identity = uid.is_some() || gid.is_some();
    // Deliberately does the whole drop inside one pre_exec closure rather than via
    // Command::uid()/gid(): those call setuid() internally as soon as they're applied,
    // and once uid is dropped the process can no longer call setgroups()/setgid() (both
    // require privileges we'd have already given away) — clear groups, then gid, then
    // uid, all while the child still has the parent's (root) privileges. Resource limits
    // only ever narrow (never require elevated privilege to set), so their order
    // relative to the identity drop doesn't matter; applied after for readability.
    //
    // SAFETY: only async-signal-safe libc calls (setgroups/setgid/setuid/setrlimit) are
    // made, and this closure runs in the forked child between fork() and exec(), before
    // any other code (including allocation-heavy Rust runtime machinery) can interfere.
    unsafe {
        cmd.pre_exec(move || {
            if drop_identity {
                nix::unistd::setgroups(&[])?;
                if let Some(gid) = gid {
                    nix::unistd::setgid(nix::unistd::Gid::from_raw(gid))?;
                }
                if let Some(uid) = uid {
                    nix::unistd::setuid(nix::unistd::Uid::from_raw(uid))?;
                }
            }
            if let Some(bytes) = max_memory_bytes {
                set_rlimit(rustix::process::Resource::As, bytes)?;
            }
            if let Some(seconds) = max_cpu_seconds {
                set_rlimit(rustix::process::Resource::Cpu, seconds)?;
            }
            if let Some(count) = max_processes {
                set_rlimit(rustix::process::Resource::Nproc, count)?;
            }
            // Applied last, right before exec: constrains the process's final
            // state rather than something the identity drop/rlimits above
            // could still need to work around.
            if seccomp_enabled {
                apply_seccomp_denylist()?;
            }
            Ok(())
        });
    }
}

#[cfg(target_os = "linux")]
fn set_rlimit(resource: rustix::process::Resource, value: u64) -> std::io::Result<()> {
    let limit = rustix::process::Rlimit {
        current: Some(value),
        maximum: Some(value),
    };
    rustix::process::setrlimit(resource, limit).map_err(std::io::Error::from)
}

/// Syscalls specifically dangerous for a sandboxed subprocess to have (privilege
/// escalation, container/namespace escape, kernel-module loading, tracing another
/// process, ...) - deliberately a denylist, not an allowlist: plugins are arbitrary
/// external scripts/binaries with unknowable syscall needs, so shipping a strict
/// allowlist as a default would be unsafe to turn on. Matches how Docker's own
/// default seccomp profile works (broad allow, block the specifically dangerous
/// syscalls), layered on top of the uid/gid drop and rlimits above as defense in
/// depth, not a replacement for them.
#[cfg(target_os = "linux")]
const SECCOMP_DENYLIST: &[i64] = &[
    libc::SYS_ptrace,
    libc::SYS_mount,
    libc::SYS_umount2,
    libc::SYS_pivot_root,
    libc::SYS_reboot,
    libc::SYS_kexec_load,
    libc::SYS_init_module,
    libc::SYS_finit_module,
    libc::SYS_delete_module,
    libc::SYS_acct,
    libc::SYS_swapon,
    libc::SYS_swapoff,
    libc::SYS_bpf,
    libc::SYS_perf_event_open,
    libc::SYS_keyctl,
    libc::SYS_add_key,
    libc::SYS_request_key,
    libc::SYS_setns,
    libc::SYS_unshare,
];

/// Installs a seccomp-bpf filter on the calling (post-fork, pre-exec) process that
/// returns EPERM for every syscall in `SECCOMP_DENYLIST` and allows everything else -
/// a clean, loggable syscall failure for a blocked plugin rather than an opaque
/// SIGSYS kill.
#[cfg(target_os = "linux")]
fn apply_seccomp_denylist() -> std::io::Result<()> {
    use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, TargetArch};
    use std::collections::BTreeMap;

    let rules: BTreeMap<i64, Vec<seccompiler::SeccompRule>> = SECCOMP_DENYLIST
        .iter()
        .map(|&syscall| (syscall, Vec::new()))
        .collect();
    let target_arch: TargetArch = std::env::consts::ARCH
        .try_into()
        .map_err(std::io::Error::other)?;
    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EPERM as u32),
        target_arch,
    )
    .map_err(std::io::Error::other)?;
    let program: BpfProgram = filter.try_into().map_err(std::io::Error::other)?;
    seccompiler::apply_filter(&program).map_err(std::io::Error::other)
}

#[cfg(not(target_os = "linux"))]
fn apply_process_hardening(_cmd: &mut Command, _cfg: &Config) {}

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
    apply_process_hardening(&mut cmd, cfg);

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

pub async fn scheduler_loop(
    state: Arc<AppState>,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let mut ticker = interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut last_sampled: HashMap<String, Instant> = HashMap::new();

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("sensor plugin scheduler shutting down");
                break;
            }
            _ = ticker.tick() => {}
        }
        let config = state.config.load_full();
        for manifest in discover(&config) {
            if !manifest.enabled {
                continue;
            }
            let cadence = Duration::from_secs(
                manifest
                    .poll_interval_seconds
                    .unwrap_or(config.plugins.sample_interval_seconds)
                    .max(1),
            );
            if let Some(last) = last_sampled.get(&manifest.name) {
                if last.elapsed() < cadence {
                    continue;
                }
            }

            let sample = sample(&config, &manifest).await;
            if sample.ok {
                info!(plugin = %manifest.name, sensor = %sample.sensor_id, "sensor sample collected");
            } else {
                warn!(plugin = %manifest.name, error = ?sample.error, "sensor sample failed");
            }
            state.record_sample(sample).await;
            last_sampled.insert(manifest.name.clone(), Instant::now());
        }
    }
    Ok(())
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn manifest(command: &str) -> PluginManifest {
        PluginManifest {
            name: "test-plugin".into(),
            version: "0.1.0".into(),
            command: command.into(),
            args: vec![],
            capabilities: vec![],
            sensor_id: None,
            description: String::new(),
            enabled: true,
            poll_interval_seconds: None,
            publish_to_nodra: true,
        }
    }

    /// A fresh scratch directory per test, cleaned up on drop.
    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir()
                .join(format!("zyvor-plugins-test-{}-{id}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn executable(&self, name: &str, mode: u32) -> String {
            let path = self.0.join(name);
            fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            path.to_string_lossy().into_owned()
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn rejects_non_absolute_command_when_required() {
        let mut cfg = Config::default();
        cfg.plugins.require_absolute_command = true;
        let errors = validate(&cfg, &manifest("relative/path.sh"));
        assert!(errors.iter().any(|e| e.contains("absolute path")));
    }

    #[test]
    fn accepts_absolute_executable_command() {
        let dir = TempDir::new();
        let path = dir.executable("plugin.sh", 0o755);
        let cfg = Config::default();
        let errors = validate(&cfg, &manifest(&path));
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn rejects_missing_command() {
        let cfg = Config::default();
        let errors = validate(&cfg, &manifest("/nonexistent/zyvor-test-plugin"));
        assert!(errors.iter().any(|e| e.contains("unavailable")));
    }

    #[test]
    fn rejects_non_executable_command() {
        let dir = TempDir::new();
        let path = dir.executable("plugin.sh", 0o644);
        let cfg = Config::default();
        let errors = validate(&cfg, &manifest(&path));
        assert!(errors.iter().any(|e| e.contains("not executable")));
    }

    #[test]
    fn rejects_world_writable_command_when_configured() {
        let dir = TempDir::new();
        let path = dir.executable("plugin.sh", 0o777);
        let mut cfg = Config::default();
        cfg.plugins.reject_world_writable = true;
        let errors = validate(&cfg, &manifest(&path));
        assert!(errors.iter().any(|e| e.contains("world-writable")));
    }

    #[test]
    fn allows_world_writable_command_when_not_rejecting() {
        let dir = TempDir::new();
        let path = dir.executable("plugin.sh", 0o777);
        let mut cfg = Config::default();
        cfg.plugins.reject_world_writable = false;
        let errors = validate(&cfg, &manifest(&path));
        assert!(!errors.iter().any(|e| e.contains("world-writable")));
    }

    #[test]
    fn rejects_owner_not_in_allowlist() {
        let dir = TempDir::new();
        let path = dir.executable("plugin.sh", 0o755);
        let mut cfg = Config::default();
        // Our own uid is guaranteed not to be this reserved-range value.
        cfg.plugins.allowed_owners = vec!["1".to_string()];
        let errors = validate(&cfg, &manifest(&path));
        assert!(errors.iter().any(|e| e.contains("allowed_owners")));
    }

    #[test]
    fn accepts_owner_in_allowlist() {
        let dir = TempDir::new();
        let path = dir.executable("plugin.sh", 0o755);
        let mut cfg = Config::default();
        let own_uid = nix::unistd::Uid::current().as_raw();
        cfg.plugins.allowed_owners = vec![own_uid.to_string()];
        let errors = validate(&cfg, &manifest(&path));
        assert!(!errors.iter().any(|e| e.contains("allowed_owners")));
    }

    #[test]
    fn rejects_command_outside_allowed_directories() {
        let dir = TempDir::new();
        let path = dir.executable("plugin.sh", 0o755);
        let mut cfg = Config::default();
        cfg.plugins.allowed_directories = vec!["/definitely/not/here".to_string()];
        let errors = validate(&cfg, &manifest(&path));
        assert!(errors.iter().any(|e| e.contains("allowed_directories")));
    }

    #[test]
    fn accepts_command_inside_allowed_directories() {
        let dir = TempDir::new();
        let path = dir.executable("plugin.sh", 0o755);
        let mut cfg = Config::default();
        // validate() compares against the canonicalized command path (to defeat symlink
        // tricks), so the allowlist entry must be canonicalized too - e.g. macOS's /tmp
        // is itself a symlink to /private/tmp, which would otherwise never match.
        let canonical_dir = fs::canonicalize(&dir.0).unwrap();
        cfg.plugins.allowed_directories = vec![canonical_dir.to_string_lossy().into_owned()];
        let errors = validate(&cfg, &manifest(&path));
        assert!(
            !errors.iter().any(|e| e.contains("allowed_directories")),
            "unexpected errors: {errors:?}"
        );
    }

    #[test]
    fn rejects_empty_name_and_version() {
        let dir = TempDir::new();
        let path = dir.executable("plugin.sh", 0o755);
        let cfg = Config::default();
        let mut bad = manifest(&path);
        bad.name = "  ".into();
        bad.version = "".into();
        let errors = validate(&cfg, &bad);
        assert!(errors.iter().any(|e| e.contains("name is empty")));
        assert!(errors.iter().any(|e| e.contains("version is empty")));
    }

    #[test]
    fn hardening_not_needed_when_unconfigured() {
        assert!(!hardening_is_needed(&PluginConfig::default()));
    }

    #[test]
    fn hardening_needed_for_uid_only() {
        let cfg = PluginConfig {
            run_as_uid: Some(1500),
            ..Default::default()
        };
        assert!(hardening_is_needed(&cfg));
    }

    #[test]
    fn hardening_needed_for_gid_only() {
        let cfg = PluginConfig {
            run_as_gid: Some(1500),
            ..Default::default()
        };
        assert!(hardening_is_needed(&cfg));
    }

    #[test]
    fn hardening_needed_for_rlimits_only() {
        let cfg = PluginConfig {
            max_memory_bytes: Some(1024),
            ..Default::default()
        };
        assert!(hardening_is_needed(&cfg));

        let cfg = PluginConfig {
            max_cpu_seconds: Some(5),
            ..Default::default()
        };
        assert!(hardening_is_needed(&cfg));

        let cfg = PluginConfig {
            max_processes: Some(4),
            ..Default::default()
        };
        assert!(hardening_is_needed(&cfg));
    }

    #[test]
    fn hardening_needed_for_seccomp_only() {
        let cfg = PluginConfig {
            seccomp_enabled: true,
            ..Default::default()
        };
        assert!(hardening_is_needed(&cfg));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn seccomp_denylist_allows_normal_command_execution() {
        // Real child process (not calling apply_seccomp_denylist() in this test
        // process itself, which would poison the whole multi-threaded test
        // runner with a seccomp filter it can never remove) - proves the
        // denylist doesn't break the ordinary exec/write/exit path a plugin
        // needs, only the specifically dangerous syscalls it targets.
        // Fully qualified: this file's own `Command` import is tokio's async
        // variant (needed by apply_process_hardening's real callers), but this
        // test only needs a plain synchronous child process - which needs the
        // std (not tokio) CommandExt trait in scope for pre_exec.
        use std::os::unix::process::CommandExt as _;
        let mut cmd = std::process::Command::new("/bin/sh");
        cmd.arg("-c").arg("echo seccomp-ok");
        unsafe {
            cmd.pre_exec(apply_seccomp_denylist);
        }
        let output = cmd.output().unwrap();
        assert!(output.status.success(), "status: {:?}", output.status);
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "seccomp-ok");
    }
}
