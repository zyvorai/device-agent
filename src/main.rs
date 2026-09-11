// SPDX-License-Identifier: Apache-2.0

mod api;
mod config;
mod hardware;
mod integrations;
mod model;
mod plugins;
mod profile;
mod state;

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Context;
use clap::{Parser, Subcommand};
use tokio::{
    net::TcpListener,
    time::{interval, MissedTickBehavior},
};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::{config::Config, state::AppState};

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
        Command::Serve => serve(cfg).await,
    }
}

async fn serve(cfg: Config) -> anyhow::Result<()> {
    let inventory = hardware::collect_inventory(&cfg).await;
    let state = Arc::new(AppState::new(cfg.clone(), inventory));

    spawn_inventory_refresh(state.clone());
    spawn_plugin_scheduler(state.clone());

    if cfg.nodra.enabled {
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(error) = integrations::nodra::publisher_loop(state).await {
                warn!(?error, "Nodra publisher stopped");
            }
        });
    }

    let app = api::router(state);
    let addr: SocketAddr = cfg.server.listen.parse().context("invalid server.listen")?;
    let listener = TcpListener::bind(addr).await?;
    info!(%addr, "Zyvor Device Agent v{} listening", env!("CARGO_PKG_VERSION"));
    axum::serve(listener, app).await?;
    Ok(())
}

fn spawn_inventory_refresh(state: Arc<AppState>) {
    tokio::spawn(async move {
        let refresh_seconds = state.config.device.inventory_refresh_seconds.max(1);
        let mut ticker = interval(Duration::from_secs(refresh_seconds));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let inventory = hardware::collect_inventory(&state.config).await;
            state.update_inventory(inventory).await;
        }
    });
}

fn spawn_plugin_scheduler(state: Arc<AppState>) {
    tokio::spawn(async move {
        if let Err(error) = plugins::scheduler_loop(state).await {
            warn!(?error, "sensor plugin scheduler stopped");
        }
    });
}
