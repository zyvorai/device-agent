// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::{hardware, model::IntegrationStatus, plugins, state::AppState};

pub fn router(state: Arc<AppState>) -> Router {
    let dashboard_dir = state.config.server.dashboard_dir.clone();
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/inventory", get(inventory))
        .route("/api/v1/hardware", get(inventory))
        .route("/api/v1/interfaces", get(interfaces))
        .route("/api/v1/thermal", get(thermal))
        .route("/api/v1/integrations", get(integrations))
        .route("/api/v1/integrations/fleet/inventory", get(fleet_inventory))
        .route("/api/v1/plugins", get(list_plugins))
        .route("/api/v1/plugins/{name}/sample", post(sample_plugin))
        .route("/api/v1/doctor", get(doctor))
        .route("/metrics", get(metrics))
        .fallback_service(ServeDir::new(dashboard_dir).append_index_html_on_directories(true))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let current = hardware::collect_inventory(&state.config).await;
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "serial": current.device.serial,
        "uptime_seconds": current.system.uptime_seconds
    }))
}

async fn inventory(State(state): State<Arc<AppState>>) -> Json<crate::model::Inventory> {
    Json(hardware::collect_inventory(&state.config).await)
}

async fn interfaces(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let current = hardware::collect_inventory(&state.config).await;
    Json(serde_json::json!({
        "network": current.network,
        "buses": current.buses,
        "usb": current.usb
    }))
}

async fn thermal(State(state): State<Arc<AppState>>) -> Json<Vec<crate::model::ThermalZone>> {
    Json(hardware::collect_inventory(&state.config).await.thermal)
}

async fn integrations(State(state): State<Arc<AppState>>) -> Json<IntegrationStatus> {
    Json(IntegrationStatus {
        nodra_enabled: state.config.nodra.enabled,
        nodra_connected: state.nodra_connected(),
        fleet_enabled: state.config.fleet.enabled,
        fleet_projection_ready: state.config.fleet.enabled && state.config.fleet.mode == "projection",
    })
}

async fn fleet_inventory(State(state): State<Arc<AppState>>) -> Json<crate::integrations::fleet::FleetInventoryProjection> {
    let inventory = hardware::collect_inventory(&state.config).await;
    Json(crate::integrations::fleet::project(&inventory))
}

async fn list_plugins(State(state): State<Arc<AppState>>) -> Json<Vec<plugins::PluginManifest>> {
    Json(plugins::discover(&state.config))
}

async fn sample_plugin(
    Path(name): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let list = plugins::discover(&state.config);
    let Some(plugin) = list.iter().find(|p| p.name == name) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"plugin not found"})),
        ).into_response();
    };
    (
        StatusCode::OK,
        Json(serde_json::to_value(plugins::sample(&state.config, plugin).await).unwrap_or_default()),
    ).into_response()
}

async fn doctor(State(state): State<Arc<AppState>>) -> Json<crate::model::DoctorReport> {
    Json(hardware::doctor(&state.config).await)
}

async fn metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let current = hardware::collect_inventory(&state.config).await;
    let temp = current.thermal.iter().find_map(|z| z.celsius).unwrap_or(f64::NAN);
    let body = format!(
        "# HELP zyvor_device_agent_up Whether the device agent is serving requests.\n\
# TYPE zyvor_device_agent_up gauge\n\
zyvor_device_agent_up 1\n\
# HELP zyvor_device_agent_temperature_celsius First available Linux thermal zone.\n\
# TYPE zyvor_device_agent_temperature_celsius gauge\n\
zyvor_device_agent_temperature_celsius {temp}\n\
# HELP zyvor_device_agent_network_interfaces Number of Linux network interfaces.\n\
# TYPE zyvor_device_agent_network_interfaces gauge\n\
zyvor_device_agent_network_interfaces {}\n\
# HELP zyvor_device_agent_nodra_connected Nodra MQTT connection state.\n\
# TYPE zyvor_device_agent_nodra_connected gauge\n\
zyvor_device_agent_nodra_connected {}\n\
# HELP zyvor_device_agent_fleet_projection_ready Fleet-compatible local inventory projection is enabled.\n\
# TYPE zyvor_device_agent_fleet_projection_ready gauge\n\
zyvor_device_agent_fleet_projection_ready {}\n",
        current.network.len(),
        u8::from(state.nodra_connected()),
        u8::from(state.config.fleet.enabled && state.config.fleet.mode == "projection"),
    );
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], body)
}
