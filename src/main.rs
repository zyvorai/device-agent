// SPDX-License-Identifier: Apache-2.0

mod api;
mod config;
mod hardware;
mod integrations;
mod model;
mod plugins;
mod profile;
mod state;

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::Context;
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::{config::Config, state::AppState};

#[derive(Debug, Parser)]
#[command(name = "zyvor-device-agent", version, about = "Zyvor hardware edge agent")]
struct Cli {
    #[arg(long, env = "ZYVOR_DEVICE_AGENT_CONFIG", default_value = "/etc/zyvor/device-agent.toml")]
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
    /// Run local health and interface diagnostics.
    Doctor,
    /// Print the local inventory projection consumed by Zyvor Fleet agent.
    FleetInventory,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "zyvor_device_agent=info,tower_http=info".into()))
        .init();

    let cli = Cli::parse();
    let cfg = Config::load_or_default(&cli.config)?;

    match cli.command.unwrap_or(Command::Serve) {
        Command::Inventory => {
            let inventory = hardware::collect_inventory(&cfg).await;
            println!("{}", serde_json::to_string_pretty(&inventory)?);
            Ok(())
        }
        Command::Doctor => {
            let report = hardware::doctor(&cfg).await;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if report.ok { Ok(()) } else { anyhow::bail!("one or more checks failed") }
        }
        Command::FleetInventory => {
            let inventory = hardware::collect_inventory(&cfg).await;
            let projection = integrations::fleet::project(&inventory);
            println!("{}", serde_json::to_string_pretty(&projection)?);
            Ok(())
        }
        Command::Serve => serve(cfg).await,
    }
}

async fn serve(cfg: Config) -> anyhow::Result<()> {
    let inventory = hardware::collect_inventory(&cfg).await;
    let state = Arc::new(AppState::new(cfg.clone(), inventory));

    if cfg.nodra.enabled {
        let s = state.clone();
        tokio::spawn(async move {
            if let Err(error) = integrations::nodra::publisher_loop(s).await {
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
