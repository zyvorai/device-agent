// SPDX-License-Identifier: Apache-2.0

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Context;
use axum::http::{header, HeaderValue, Method};
use clap::{Parser, Subcommand};
use tokio::{
    net::TcpListener,
    time::{interval, MissedTickBehavior},
};
use tokio_util::sync::CancellationToken;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use zyvor_device_agent::{
    api, auth, camera_capture, can_capture, config::Config, hardware, identity::DeviceIdentity,
    integrations, plugins, state::AppState, tls,
};

#[derive(Debug, Parser)]
#[command(
    name = "zyvor-device-agent",
    version,
    about = "Zyvor hardware edge agent"
)]
struct Cli {
    #[arg(
        long,
        env = "ZYVOR_DEVICE_AGENT_CONFIG",
        default_value = "/etc/zyvor/device-agent.toml"
    )]
    config: PathBuf,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the local REST API, dashboard and background publishers.
    Serve,
    /// Print detected hardware inventory as JSON.
    Inventory,
    /// Print CAN and serial/RS485 hardware state as JSON.
    Industrial,
    /// Run local health and interface diagnostics.
    Doctor,
    /// Print the local inventory projection consumed by Zyvor Fleet agent.
    FleetInventory,
    /// List configured sensor plugins and their validation state.
    Plugins,
    /// Execute one configured sensor plugin immediately.
    Sample {
        /// Plugin manifest name.
        name: String,
    },
    /// Generate a CSR and submit it to `enrollment.server_url`, persisting the
    /// issued certificate/key for `auth.mode = "mtls"`.
    Enroll {
        /// Reissue even if a certificate already exists at `auth.mtls.cert_file`.
        #[arg(long)]
        force: bool,
    },
    /// Print this device's current mTLS identity (subject, validity, backend).
    Identity,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "zyvor_device_agent=info,tower_http=info".into()),
        )
        .init();

    let cli = Cli::parse();
    let cfg = Config::load_or_default(&cli.config)?;

    match cli.command.unwrap_or(Command::Serve) {
        Command::Inventory => {
            let inventory = hardware::collect_inventory(&cfg).await;
            println!("{}", serde_json::to_string_pretty(&inventory)?);
            Ok(())
        }
        Command::Industrial => {
            let industrial = hardware::industrial_inventory(&cfg).await;
            println!("{}", serde_json::to_string_pretty(&industrial)?);
            Ok(())
        }
        Command::Doctor => {
            let report = hardware::doctor(&cfg).await;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if report.ok {
                Ok(())
            } else {
                anyhow::bail!("one or more checks failed")
            }
        }
        Command::FleetInventory => {
            let inventory = hardware::collect_inventory(&cfg).await;
            let projection = integrations::fleet::project(&inventory);
            println!("{}", serde_json::to_string_pretty(&projection)?);
            Ok(())
        }
        Command::Plugins => {
            println!(
                "{}",
                serde_json::to_string_pretty(&plugins::statuses(&cfg))?
            );
            Ok(())
        }
        Command::Sample { name } => {
            let manifests = plugins::discover(&cfg);
            let manifest = manifests
                .iter()
                .find(|plugin| plugin.name == name)
                .with_context(|| format!("sensor plugin {name:?} not found"))?;
            let sample = plugins::sample(&cfg, manifest).await;
            println!("{}", serde_json::to_string_pretty(&sample)?);
            if sample.ok {
                Ok(())
            } else {
                anyhow::bail!("sensor sample failed")
            }
        }
        Command::Enroll { force } => auth::enroll::run(&cfg, force).await,
        Command::Identity => print_identity(&cfg),
        Command::Serve => serve(cfg, cli.config).await,
    }
}

fn print_identity(cfg: &Config) -> anyhow::Result<()> {
    let cert_path = &cfg.auth.mtls.cert_file;
    let pem = std::fs::read(cert_path)
        .with_context(|| format!("reading {cert_path} (run `enroll` first?)"))?;
    let (_, pem) =
        x509_parser::pem::parse_x509_pem(&pem).context("parsing device certificate PEM")?;
    let cert = pem.parse_x509().context("parsing device certificate DER")?;
    let backend = DeviceIdentity::load(cfg, &cfg.auth.mtls.key_file)
        .map(|identity| identity.backend_name())
        .unwrap_or("unknown (key file unreadable)");
    println!("cert_file:  {cert_path}");
    println!("backend:    {backend}");
    println!("subject:    {}", cert.subject());
    println!("issuer:     {}", cert.issuer());
    println!("not_before: {}", cert.validity().not_before);
    println!("not_after:  {}", cert.validity().not_after);
    Ok(())
}

async fn serve(cfg: Config, config_path: PathBuf) -> anyhow::Result<()> {
    if cfg.auth.mode == "mtls"
        && (!std::path::Path::new(&cfg.auth.mtls.cert_file).exists()
            || !std::path::Path::new(&cfg.auth.mtls.key_file).exists())
    {
        anyhow::bail!(
            "auth.mode = \"mtls\" but no identity found at {} / {} - run `zyvor-device-agent enroll` first",
            cfg.auth.mtls.cert_file,
            cfg.auth.mtls.key_file
        );
    }

    let inventory = hardware::collect_inventory(&cfg).await;
    let state = Arc::new(AppState::new(cfg.clone(), inventory));
    let shutdown = CancellationToken::new();

    spawn_inventory_refresh(state.clone(), shutdown.clone());
    spawn_plugin_scheduler(state.clone(), shutdown.clone());
    spawn_config_reload_listener(state.clone(), config_path, shutdown.clone());
    can_capture::spawn(state.clone(), shutdown.clone());
    camera_capture::spawn(state.clone(), shutdown.clone());
    hardware::hotplug::spawn(state.clone(), shutdown.clone());

    if cfg.nodra.enabled {
        let state = state.clone();
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            if let Err(error) = integrations::nodra::publisher_loop(state, shutdown).await {
                warn!(?error, "Nodra publisher stopped");
            }
        });
    }

    if cfg.server.unix_socket.enabled {
        spawn_uds_listener(
            &cfg,
            api::router(state.clone()),
            state.clone(),
            shutdown.clone(),
        )?;
    }

    // Rate limiting and CORS apply only to this network-facing TCP router, not the
    // Unix-socket one above: neither concept applies to a same-host UDS connection
    // (no peer IP for the rate limiter's key, no browser origin to check).
    let mut app = api::router(state);
    if let Some(cors) = build_cors_layer(&cfg.server.cors) {
        app = app.layer(cors);
    }
    if cfg.server.rate_limit.enabled {
        let rl = &cfg.server.rate_limit;
        let period_ms = (1000 / rl.requests_per_second.max(1)).max(1);
        let governor_conf = Arc::new(
            GovernorConfigBuilder::default()
                .per_millisecond(u64::from(period_ms))
                .burst_size(rl.burst.max(1))
                .finish()
                .context("invalid server.rate_limit configuration")?,
        );
        app = app.layer(GovernorLayer {
            config: governor_conf,
        });
    }

    let addr: SocketAddr = cfg.server.listen.parse().context("invalid server.listen")?;
    info!(%addr, "Zyvor Device Agent v{} listening", env!("CARGO_PKG_VERSION"));

    // `auth.mode = "mtls"` and `server.tls.enabled` are independent TLS
    // sources: the former ties TLS to the mTLS identity/client-verification
    // system (see `auth::mtls`), the latter is plain server-side TLS (no
    // client cert ever required) via a self-signed-by-default cert (see
    // `tls::ensure_self_signed_cert`). At most one applies; `auth.mode =
    // "mtls"` wins if both are somehow set, since it's the more specific
    // configuration.
    let rustls_config = if cfg.auth.mode == "mtls" {
        Some(auth::mtls::load_server_config(&cfg).await?)
    } else if cfg.server.tls.enabled {
        let tls = &cfg.server.tls;
        tls::ensure_self_signed_cert(&tls.cert_path, &tls.key_path)?;
        Some(
            axum_server::tls_rustls::RustlsConfig::from_pem_file(&tls.cert_path, &tls.key_path)
                .await
                .with_context(|| {
                    format!("loading TLS cert {} / key {}", tls.cert_path, tls.key_path)
                })?,
        )
    } else {
        None
    };

    match rustls_config {
        Some(rustls_config) => {
            let handle = axum_server::Handle::new();
            tokio::spawn({
                let handle = handle.clone();
                let shutdown = shutdown.clone();
                async move {
                    shutdown_signal(shutdown).await;
                    // Matches the systemd unit's TimeoutStopSec=15 for the plain-TCP path.
                    handle.graceful_shutdown(Some(Duration::from_secs(15)));
                }
            });
            axum_server::bind_rustls(addr, rustls_config)
                .handle(handle)
                .serve(app.into_make_service_with_connect_info::<SocketAddr>())
                .await?;
        }
        None => {
            let listener = TcpListener::bind(addr).await?;
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown_signal(shutdown.clone()))
            .await?;
        }
    }
    info!("shutdown complete");
    Ok(())
}

fn build_cors_layer(cfg: &zyvor_device_agent::config::CorsConfig) -> Option<CorsLayer> {
    if !cfg.enabled {
        return None;
    }
    let origins: Vec<HeaderValue> = cfg
        .allowed_origins
        .iter()
        .filter_map(|origin| match origin.parse::<HeaderValue>() {
            Ok(value) => Some(value),
            Err(error) => {
                warn!(%origin, %error, "invalid server.cors.allowed_origins entry, skipping");
                None
            }
        })
        .collect();
    Some(
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([Method::GET, Method::POST])
            .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]),
    )
}

/// Resolves once SIGINT/SIGTERM is received, or the shared token is otherwise
/// cancelled - and cancels the token itself, so callers only need to await
/// either this future (for axum's `with_graceful_shutdown`) or
/// `shutdown.cancelled()` (for background loops), whichever fires first.
async fn shutdown_signal(shutdown: CancellationToken) {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        else {
            std::future::pending::<()>().await;
            return;
        };
        signal.recv().await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("received SIGINT, shutting down gracefully"),
        _ = terminate => info!("received SIGTERM, shutting down gracefully"),
        _ = shutdown.cancelled() => {}
    }
    shutdown.cancel();
}

fn spawn_uds_listener(
    cfg: &Config,
    base_router: axum::Router,
    state: Arc<AppState>,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let (listener, router) = auth::uds::bind(&cfg.server.unix_socket, base_router, state)?;
    let path = cfg.server.unix_socket.path.clone();
    info!(%path, "Zyvor Device Agent Unix socket API listening");
    tokio::spawn(async move {
        if let Err(error) = axum::serve(
            listener,
            router.into_make_service_with_connect_info::<auth::uds::PeerCred>(),
        )
        .with_graceful_shutdown(async move { shutdown.cancelled().await })
        .await
        {
            warn!(?error, "Unix socket listener stopped");
        }
    });
    Ok(())
}

fn spawn_inventory_refresh(state: Arc<AppState>, shutdown: CancellationToken) {
    tokio::spawn(async move {
        // Ticker cadence is captured once: changing device.inventory_refresh_seconds
        // needs a restart to re-time it. hardware::collect_inventory below reads the
        // live config on every tick, so most other device.*/industrial.* fields do
        // hot-reload correctly.
        let refresh_seconds = state.config.load().device.inventory_refresh_seconds.max(1);
        let mut ticker = interval(Duration::from_secs(refresh_seconds));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("inventory refresh loop shutting down");
                    break;
                }
                _ = ticker.tick() => {
                    let inventory = hardware::collect_inventory(&state.config.load()).await;
                    state.update_inventory(inventory).await;
                }
            }
        }
    });
}

fn spawn_plugin_scheduler(state: Arc<AppState>, shutdown: CancellationToken) {
    tokio::spawn(async move {
        if let Err(error) = plugins::scheduler_loop(state, shutdown).await {
            warn!(?error, "sensor plugin scheduler stopped");
        }
    });
}

/// SIGHUP re-reads the config file and hot-swaps `state.config` - see
/// `AppState::reload_config` for exactly which fields take effect immediately
/// versus needing a restart. A config file that fails to parse is logged and
/// ignored, keeping the daemon on its last-known-good config rather than
/// crashing or running with a half-applied reload.
#[cfg(unix)]
fn spawn_config_reload_listener(
    state: Arc<AppState>,
    config_path: PathBuf,
    shutdown: CancellationToken,
) {
    tokio::spawn(async move {
        let Ok(mut signal) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
        else {
            warn!("failed to install SIGHUP handler; config hot-reload unavailable");
            return;
        };
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => break,
                received = signal.recv() => {
                    if received.is_none() {
                        break;
                    }
                    info!(path = %config_path.display(), "received SIGHUP, reloading config");
                    match Config::load_or_default(&config_path) {
                        Ok(new_config) => {
                            let restart_needed = state.reload_config(new_config);
                            if restart_needed {
                                warn!(
                                    "config reloaded, but server.listen/unix_socket/dashboard_dir \
                                     changed and need a full restart to take effect"
                                );
                            } else {
                                info!("config reloaded");
                            }
                        }
                        Err(error) => {
                            warn!(%error, "SIGHUP config reload failed; keeping the current config");
                        }
                    }
                }
            }
        }
    });
}

#[cfg(not(unix))]
fn spawn_config_reload_listener(
    _state: Arc<AppState>,
    _config_path: PathBuf,
    _shutdown: CancellationToken,
) {
}
