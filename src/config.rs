// SPDX-License-Identifier: Apache-2.0

use std::{fs, path::Path};

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub enrollment: EnrollmentConfig,
    pub identity: IdentityConfig,
    pub device: DeviceConfig,
    pub industrial: IndustrialConfig,
    pub nodra: NodraConfig,
    pub fleet: FleetConfig,
    pub plugins: PluginConfig,
    pub thresholds: ThresholdConfig,
    pub camera: CameraConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThresholdConfig {
    /// Off by default: purely additive alerting on top of the existing raw
    /// thermal/CAN-controller-error readings, opt-in like every other
    /// hardening field in this config.
    pub enabled: bool,
    pub thermal_warn_celsius: f64,
    pub thermal_critical_celsius: f64,
    /// SocketCAN controller error counters (0-255, standard CAN semantics):
    /// 96 is the conventional error-warning level, 128 is error-passive.
    pub can_error_counter_warn: u64,
    pub can_error_counter_critical: u64,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            thermal_warn_celsius: 75.0,
            thermal_critical_celsius: 90.0,
            can_error_counter_warn: 96,
            can_error_counter_critical: 128,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub listen: String,
    pub dashboard_dir: String,
    pub unix_socket: UnixSocketConfig,
    pub cors: CorsConfig,
    pub rate_limit: RateLimitConfig,
    pub tls: TlsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TlsConfig {
    /// Off by default — a plain `cargo build`/existing deployment keeps
    /// serving plain HTTP unless this is explicitly turned on. Independent
    /// of `auth.mode`: unlike `auth.mode = "mtls"`, this never requires a
    /// client certificate, it only encrypts the connection and proves the
    /// daemon's own identity. See `src/tls.rs`.
    pub enabled: bool,
    /// If missing at startup and `enabled`, a self-signed cert is
    /// generated here automatically. Mount a real cert at the same path to
    /// use one instead — an existing cert/key here is never overwritten.
    pub cert_path: String,
    pub key_path: String,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cert_path: "/etc/zyvor/device-agent/tls/server.crt".into(),
            key_path: "/etc/zyvor/device-agent/tls/server.key".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CorsConfig {
    /// Off by default: the bundled dashboard is served same-origin (see
    /// `api::router`'s `ServeDir` fallback) and needs no CORS layer at all.
    /// Enable only for a cross-origin dashboard/integration.
    pub enabled: bool,
    pub allowed_origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RateLimitConfig {
    /// On by default with a generous limit: pure DoS protection with no
    /// behavior change for normal traffic, unlike auth/CORS which change
    /// what a legitimate caller has to do.
    pub enabled: bool,
    pub requests_per_second: u32,
    pub burst: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UnixSocketConfig {
    /// Additional API listener over a Unix domain socket, alongside the TCP listener.
    pub enabled: bool,
    pub path: String,
    /// Applied to the socket file after bind.
    pub file_mode: u32,
    /// Peer uids allowed to connect. Empty (with allow_gids also empty) denies everyone —
    /// must be explicitly populated.
    pub allow_uids: Vec<u32>,
    /// Peer gids allowed to connect (matched against the peer's primary gid).
    pub allow_gids: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    /// "none" | "bearer" | "mtls" — auth mode enforced on the TCP API listener.
    pub mode: String,
    /// Route paths that bypass auth entirely (e.g. health probes).
    pub exempt_paths: Vec<String>,
    pub bearer: BearerAuthConfig,
    pub mtls: MtlsAuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BearerAuthConfig {
    /// Path to a file containing the SHA-256 hash (hex) of the accepted bearer token.
    /// The raw token itself is never stored by the daemon.
    pub token_hash_file: String,
}

/// `auth.mode = "mtls"`: the TCP listener terminates TLS itself (via
/// `axum-server`'s rustls integration) using the identity issued by
/// `zyvor-device-agent enroll` (see [`EnrollmentConfig`]), and requires an
/// incoming client certificate signed by `client_ca_file`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MtlsAuthConfig {
    /// This device's issued identity certificate (PEM), written by `enroll`.
    pub cert_file: String,
    /// This device's private key (PEM, PKCS#8), written by `enroll`. Never
    /// leaves disk in software mode; see `docs/ROADMAP.md` for the optional
    /// TPM-backed key provider.
    pub key_file: String,
    /// CA bundle (PEM) used to verify an *incoming* client's certificate.
    /// Distinct from `enrollment.ca_bundle_file`, which verifies the
    /// enrollment *server's* own TLS certificate during `enroll` - the two
    /// are logically separate trust decisions even when, in a single-CA
    /// deployment, they happen to be the same bundle.
    pub client_ca_file: String,
    /// If false, the listener still terminates TLS with this device's own
    /// certificate but accepts connections with no client certificate at
    /// all - effectively TLS without the "m". Off by default like every
    /// other hardening field; a real deployment sets this true.
    pub require_client_cert: bool,
}

impl Default for MtlsAuthConfig {
    fn default() -> Self {
        Self {
            cert_file: "/etc/zyvor/device-agent/identity/device-cert.pem".into(),
            key_file: "/etc/zyvor/device-agent/identity/device-key.pem".into(),
            client_ca_file: "/etc/zyvor/device-agent/identity/client-ca.pem".into(),
            require_client_cert: false,
        }
    }
}

/// Client-side enrollment: generates a CSR, submits it to a Fleet-compatible
/// enrollment endpoint, and persists the issued certificate/key for
/// `auth.mode = "mtls"`. Device Agent never issues or signs certificates
/// itself - see `docs/MTLS_ENROLLMENT.md` for the protocol and why this
/// deliberately stops at the client side.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EnrollmentConfig {
    pub enabled: bool,
    /// Base URL of the enrollment endpoint, e.g. `https://fleet.example:8443/enroll`.
    pub server_url: String,
    /// Path to a single-use enrollment token (bearer-style). Deleted on
    /// successful enrollment so it can't be replayed.
    pub token_file: String,
    /// CA bundle (PEM) used to verify the enrollment server's own TLS
    /// certificate. Empty uses the system trust store.
    pub ca_bundle_file: String,
    /// Common Name to request in the CSR. Empty defaults to `device.serial`.
    pub common_name: String,
}

impl Default for EnrollmentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: String::new(),
            token_file: "/etc/zyvor/device-agent/identity/enrollment-token".into(),
            ca_bundle_file: String::new(),
            common_name: String::new(),
        }
    }
}

/// Where the mTLS private key backing [`MtlsAuthConfig`] actually lives and
/// signs. Optional and feature-gated - see `docs/TPM2_IDENTITY.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IdentityConfig {
    /// "software" (default) or "tpm". `tpm` requires the `tpm2` build
    /// feature; falls back to `software` at runtime (with a warning) if the
    /// TPM can't be opened - most dev/test boxes have no TPM.
    pub backend: String,
    /// TCTI connection string, e.g. "device:/dev/tpmrm0" or
    /// "swtpm:host=127.0.0.1,port=2321". Empty uses tss-esapi's own
    /// environment-variable-based default resolution.
    pub tpm_tcti: String,
}

impl Default for IdentityConfig {
    fn default() -> Self {
        Self {
            backend: "software".into(),
            tpm_tcti: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceConfig {
    pub vendor: String,
    pub model: String,
    pub serial: String,
    pub profile: String,
    pub profile_directory: String,
    pub telemetry_interval_seconds: u64,
    pub inventory_refresh_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IndustrialConfig {
    /// Command used only for richer read-only SocketCAN details. Set empty to disable.
    pub can_ip_command: String,
    /// Serial ports that the board/operator declares as RS485-capable, e.g. ttyS1 or /dev/ttyS1.
    pub rs485_ports: Vec<String>,
    /// Publish industrial inventory snapshots into the Nodra MQTT namespace.
    pub publish_to_nodra: bool,
    /// Optional read-only SocketCAN frame capture. Disabled by default.
    pub can_capture: CanCaptureConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CanCaptureConfig {
    pub enabled: bool,
    pub interfaces: Vec<String>,
    pub history_limit: usize,
    pub max_frames_per_second: u32,
    pub include_error_frames: bool,
    pub publish_to_nodra: bool,
}

/// Top-level, not nested under `[industrial]` - a camera isn't a fieldbus
/// concept, and the future local-inference/accelerator work this seeds
/// (see `docs/ROADMAP.md`'s "Edge AI bridge" entry) shouldn't have to
/// retrofit out of an industrial sub-table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraConfig {
    /// Explicit per-camera declarations. No `/dev/video*` is ever opened
    /// unless listed here - same "never auto-scan" posture as
    /// `[industrial.can_capture]`'s interface allowlist.
    pub devices: Vec<CameraDeviceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraDeviceConfig {
    /// Stable id used in API paths (`/api/v1/camera/{id}/...`) - not the
    /// `/dev/videoN` path, whose numbering isn't stable across
    /// reboots/replugs.
    pub id: String,
    pub path: String,
    pub enabled: bool,
    pub max_frames_per_second: u32,
    /// 1-100. Only used on the software YUYV-encode fallback path - ignored
    /// for devices captured via native MJPG passthrough.
    pub jpeg_quality: u8,
    /// 0 = unlimited, matching this config's existing "0/unset = no limit"
    /// idiom (e.g. `PluginConfig`'s rlimit fields).
    pub max_stream_clients: u32,
    /// Publishes only `CameraCaptureStatus` (health/presence) to Nodra,
    /// never frame bytes - see `docs/CAMERA.md`.
    pub publish_to_nodra: bool,
}

impl Default for CameraDeviceConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            path: String::new(),
            enabled: false,
            max_frames_per_second: 10,
            jpeg_quality: 75,
            max_stream_clients: 4,
            publish_to_nodra: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NodraConfig {
    pub enabled: bool,
    pub broker: String,
    pub port: u16,
    pub client_id: String,
    pub topic_prefix: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub retain_inventory: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FleetConfig {
    pub enabled: bool,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginConfig {
    pub directory: String,
    pub timeout_seconds: u64,
    pub sample_interval_seconds: u64,
    pub require_absolute_command: bool,
    pub reject_world_writable: bool,
    pub max_output_bytes: usize,
    /// Drop plugin subprocesses to this uid before exec. None = inherit the daemon's uid
    /// (today's behavior). Opt-in: the target uid must have access to whatever device
    /// nodes the plugin needs (see docs/HARDWARE_PERMISSIONS.md).
    pub run_as_uid: Option<u32>,
    /// Drop plugin subprocesses to this gid before exec. None = inherit the daemon's gid.
    pub run_as_gid: Option<u32>,
    /// Only allow plugin commands owned by one of these uids (numeric, or a username
    /// resolved via /etc/passwd). Empty = no restriction (today's behavior).
    pub allowed_owners: Vec<String>,
    /// Only allow plugin commands whose canonicalized path starts with one of these
    /// prefixes. Empty = no restriction (today's behavior).
    pub allowed_directories: Vec<String>,
    /// RLIMIT_AS (address space) applied to plugin subprocesses. None = no limit.
    pub max_memory_bytes: Option<u64>,
    /// RLIMIT_CPU (seconds of CPU time) applied to plugin subprocesses. None = no limit.
    pub max_cpu_seconds: Option<u64>,
    /// RLIMIT_NPROC applied to plugin subprocesses, guards against a runaway plugin
    /// fork-bombing. None = no limit.
    pub max_processes: Option<u64>,
    /// Linux only: installs a seccomp-bpf denylist in the plugin subprocess blocking a
    /// fixed set of dangerous syscalls (ptrace, mount, module loading, ...) with EPERM,
    /// layered on top of run_as_uid/run_as_gid and the rlimits above as defense-in-depth
    /// - not a replacement for them. Off by default like every other hardening field.
    pub seccomp_enabled: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0:9188".into(),
            dashboard_dir: "/usr/share/zyvor-device-agent/dashboard".into(),
            unix_socket: UnixSocketConfig::default(),
            cors: CorsConfig::default(),
            rate_limit: RateLimitConfig::default(),
            tls: TlsConfig::default(),
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_second: 20,
            burst: 40,
        }
    }
}

impl Default for UnixSocketConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: "/run/zyvor-device-agent/api.sock".into(),
            file_mode: 0o660,
            allow_uids: Vec::new(),
            allow_gids: Vec::new(),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            mode: "none".into(),
            exempt_paths: vec!["/api/v1/health".into(), "/api/v1/ready".into()],
            bearer: BearerAuthConfig::default(),
            mtls: MtlsAuthConfig::default(),
        }
    }
}

impl Default for BearerAuthConfig {
    fn default() -> Self {
        Self {
            token_hash_file: "/etc/zyvor/device-agent/auth/bearer.sha256".into(),
        }
    }
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            vendor: "Generic Linux".into(),
            model: "Edge Gateway".into(),
            serial: "auto".into(),
            profile: "generic-linux-arm64".into(),
            profile_directory: "/etc/zyvor/device-agent/profiles".into(),
            telemetry_interval_seconds: 10,
            inventory_refresh_seconds: 5,
        }
    }
}

impl Default for IndustrialConfig {
    fn default() -> Self {
        Self {
            can_ip_command: "ip".into(),
            rs485_ports: Vec::new(),
            publish_to_nodra: true,
            can_capture: CanCaptureConfig::default(),
        }
    }
}

impl Default for CanCaptureConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interfaces: Vec::new(),
            history_limit: 512,
            max_frames_per_second: 200,
            include_error_frames: false,
            publish_to_nodra: true,
        }
    }
}

impl Default for NodraConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            broker: "127.0.0.1".into(),
            port: 1883,
            client_id: "zyvor-device-agent".into(),
            topic_prefix: "zyvor/device".into(),
            username: None,
            password: None,
            retain_inventory: true,
        }
    }
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: "projection".into(),
        }
    }
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            directory: "/etc/zyvor/device-agent/plugins.d".into(),
            timeout_seconds: 3,
            sample_interval_seconds: 5,
            require_absolute_command: true,
            reject_world_writable: true,
            max_output_bytes: 262_144,
            run_as_uid: None,
            run_as_gid: None,
            allowed_owners: Vec::new(),
            allowed_directories: Vec::new(),
            max_memory_bytes: None,
            max_cpu_seconds: None,
            max_processes: None,
            seccomp_enabled: false,
        }
    }
}

impl Config {
    pub fn load_or_default(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
    }
}
