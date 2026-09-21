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
    pub edge_ai: EdgeAiConfig,
    pub privsep: PrivsepConfig,
    pub recorder: RecorderConfig,
    pub remediation: RemediationConfig,
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
    /// Explicit opt-in to serve `auth.mode = "none"` on a non-loopback bind.
    /// Default false: startup refuses that combination.
    pub allow_unauthenticated_remote: bool,
    /// Durable agent state: recorder segments, fleet sequence, audit log.
    pub state_dir: String,
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
    /// Lifetime of a camera/SSE stream ticket. The long-lived bearer never
    /// goes on a query string.
    pub stream_ticket_ttl_seconds: u64,
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
    /// When true, `serve` asks the enrollment server whether this device
    /// certificate is revoked and refuses to start if it is.
    pub revocation_check: bool,
}

impl Default for EnrollmentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: String::new(),
            token_file: "/etc/zyvor/device-agent/identity/enrollment-token".into(),
            ca_bundle_file: String::new(),
            common_name: String::new(),
            revocation_check: false,
        }
    }
}

/// Where the mTLS private key backing [`MtlsAuthConfig`] actually lives and
/// signs. Optional and feature-gated - see `docs/TPM2_IDENTITY.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IdentityConfig {
    /// "software" (default) or "tpm". `tpm` requires the `tpm2` build
    /// feature. Whether a TPM failure is fatal is `policy`, not this field.
    pub backend: String,
    /// "software" forces a file key. "preferred" (default) keeps today's
    /// TPM-then-software fallback. "required" fails `enroll` and `serve`
    /// unless a TPM key can be opened. Production profiles use `required`.
    pub policy: String,
    /// TCTI connection string, e.g. "device:/dev/tpmrm0" or
    /// "swtpm:host=127.0.0.1,port=2321". Empty uses tss-esapi's own
    /// environment-variable-based default resolution.
    pub tpm_tcti: String,
}

impl Default for IdentityConfig {
    fn default() -> Self {
        Self {
            backend: "software".into(),
            policy: "preferred".into(),
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

/// Local inference bridge scaffold — see `docs/EDGE_AI.md`. Parsed always so
/// TOML stays stable; the `/api/v1/inference/*` routes only exist when the
/// binary is built with `--features edge-ai`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EdgeAiConfig {
    /// Operator intent flag. Today never starts a backend even when true —
    /// handlers return 501 `not-configured` until a real accelerator family
    /// is implemented.
    pub enabled: bool,
    /// Accelerator family name placeholder (e.g. `"rknn"`, `"openvino"`,
    /// `"tensorrt"`). Empty means undeclared. Naming a family does not load
    /// a driver in this scaffold.
    pub accelerator: String,
    /// Future: publish inference event envelopes to Nodra (never raw frames).
    pub publish_to_nodra: bool,
}

impl Default for EdgeAiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            accelerator: String::new(),
            publish_to_nodra: true,
        }
    }
}

/// Privilege-separation helper for daemon bus access — see `docs/PRIVSEP.md`.
/// Default `enabled = false` keeps the stock root daemon path unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivsepConfig {
    /// Opt-in. When false (default), the daemon opens buses directly.
    /// When true *and* the binary is built with `--features privsep`, the
    /// API process talks to `bus-helper` instead of opening device nodes.
    pub enabled: bool,
    /// Helper binary. If it is missing, `serve` re-executes this binary's
    /// `bus-helper` subcommand.
    pub helper_path: String,
    /// Unix socket the API daemon uses to talk to the helper.
    pub socket_path: String,
    /// Peer uids allowed to call the helper. Empty, together with
    /// `allow_gids`, means "same uid as the helper process".
    pub allow_uids: Vec<u32>,
    /// Peer gids allowed to call the helper.
    pub allow_gids: Vec<u32>,
}

impl Default for PrivsepConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            helper_path: "/usr/lib/zyvor-device-agent/bus-helper".into(),
            socket_path: "/run/zyvor-device-agent/bus.sock".into(),
            allow_uids: Vec::new(),
            allow_gids: Vec::new(),
        }
    }
}

/// Bounded local operational journal. Not a Nodra application-data WAL.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RecorderConfig {
    pub enabled: bool,
    /// Empty uses `{server.state_dir}/recorder`.
    pub directory: String,
    /// Rotate the active segment after this many bytes.
    pub segment_bytes: u64,
    /// Delete the oldest segments once the directory exceeds this.
    pub max_bytes: u64,
    /// Store selected CAN frames. Off by default (privacy).
    pub can_frames: bool,
    /// When `can_frames` is on, replace payload bytes with a length.
    pub redact_can_payload: bool,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: String::new(),
            segment_bytes: 1_048_576,
            max_bytes: 32 * 1_048_576,
            can_frames: false,
            redact_can_payload: true,
        }
    }
}

/// Signed, allowlisted remediation. No shell. Off until an operator enables it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RemediationConfig {
    pub enabled: bool,
    /// PEM SubjectPublicKeyInfo trusted for Fleet command signatures.
    pub fleet_public_key_pem: String,
    /// systemd unit names `restart-service` may touch.
    pub service_allowlist: Vec<String>,
    /// CAN interfaces `reopen-can` may touch.
    pub can_allowlist: Vec<String>,
    /// Reboot and CAN reopen need a second signature when true.
    pub two_person: bool,
}

impl Default for RemediationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            fleet_public_key_pem: String::new(),
            service_allowlist: Vec::new(),
            can_allowlist: Vec::new(),
            two_person: true,
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
    /// MQTT over TLS (MQTTS). Plain MQTT remains for trusted LAN / loopback only.
    pub tls: NodraTlsConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NodraTlsConfig {
    pub enabled: bool,
    /// PEM CA bundle used to verify the broker. Empty + `enabled` uses the
    /// rumqttc/webpki default root store.
    pub ca_file: String,
    /// Optional client certificate PEM for broker mTLS.
    pub cert_file: String,
    /// Optional client private key PEM (required when `cert_file` is set).
    pub key_file: String,
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
            listen: "127.0.0.1:9188".into(),
            dashboard_dir: "/usr/share/zyvor-device-agent/dashboard".into(),
            unix_socket: UnixSocketConfig::default(),
            cors: CorsConfig::default(),
            rate_limit: RateLimitConfig::default(),
            tls: TlsConfig::default(),
            allow_unauthenticated_remote: false,
            state_dir: "/var/lib/zyvor-device-agent".into(),
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
            stream_ticket_ttl_seconds: 60,
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
            tls: NodraTlsConfig::default(),
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

/// Refuse `auth.mode = "none"` on a non-loopback TCP bind unless the operator
/// set `server.allow_unauthenticated_remote`.
pub fn ensure_authenticated_bind(cfg: &Config) -> anyhow::Result<()> {
    if cfg.auth.mode != "none" || cfg.server.allow_unauthenticated_remote {
        return Ok(());
    }
    if listen_is_loopback(&cfg.server.listen)? {
        return Ok(());
    }
    anyhow::bail!(
        "refusing to serve auth.mode = \"none\" on non-loopback address {}. \
         Set auth.mode to \"bearer\" or \"mtls\", bind 127.0.0.1, or set \
         server.allow_unauthenticated_remote = true to keep the unsafe behavior",
        cfg.server.listen
    );
}

pub fn listen_is_loopback(listen: &str) -> anyhow::Result<bool> {
    let addr: std::net::SocketAddr = listen
        .parse()
        .with_context(|| format!("invalid server.listen {listen:?}"))?;
    Ok(addr.ip().is_loopback())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_ai_and_privsep_defaults_are_inert() {
        let cfg = Config::default();
        assert!(!cfg.edge_ai.enabled);
        assert!(cfg.edge_ai.accelerator.is_empty());
        assert!(!cfg.privsep.enabled);
        assert!(!cfg.privsep.helper_path.is_empty());
        assert!(!cfg.privsep.socket_path.is_empty());
        assert!(!cfg.server.allow_unauthenticated_remote);
        assert_eq!(cfg.server.listen, "127.0.0.1:9188");
        assert_eq!(cfg.identity.policy, "preferred");
        assert!(cfg.recorder.enabled);
        assert!(!cfg.remediation.enabled);
    }

    #[test]
    fn edge_ai_and_privsep_toml_round_trip() {
        let raw = r#"
[edge_ai]
enabled = true
accelerator = "rknn"
publish_to_nodra = false

[privsep]
enabled = true
helper_path = "/opt/bus-helper"
socket_path = "/tmp/bus.sock"
"#;
        let cfg: Config = toml::from_str(raw).expect("parse");
        assert!(cfg.edge_ai.enabled);
        assert_eq!(cfg.edge_ai.accelerator, "rknn");
        assert!(!cfg.edge_ai.publish_to_nodra);
        assert!(cfg.privsep.enabled);
        assert_eq!(cfg.privsep.helper_path, "/opt/bus-helper");
        assert_eq!(cfg.privsep.socket_path, "/tmp/bus.sock");
    }

    #[test]
    fn unauthenticated_non_loopback_is_refused() {
        let mut cfg = Config::default();
        cfg.server.listen = "0.0.0.0:9188".into();
        let error = ensure_authenticated_bind(&cfg).unwrap_err();
        assert!(error.to_string().contains("refusing"));
        cfg.server.allow_unauthenticated_remote = true;
        ensure_authenticated_bind(&cfg).unwrap();
        cfg.server.allow_unauthenticated_remote = false;
        cfg.auth.mode = "bearer".into();
        ensure_authenticated_bind(&cfg).unwrap();
        cfg.auth.mode = "none".into();
        cfg.server.listen = "127.0.0.1:9188".into();
        ensure_authenticated_bind(&cfg).unwrap();
    }
}
