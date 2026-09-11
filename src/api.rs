// SPDX-License-Identifier: Apache-2.0

use std::{convert::Infallible, sync::Arc, time::Duration};

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::{hardware, model::IntegrationStatus, plugins, state::AppState};

pub fn router(state: Arc<AppState>) -> Router {
    let dashboard_dir = state.config.server.dashboard_dir.clone();
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/status", get(status))
        .route("/api/v1/inventory", get(inventory))
        .route("/api/v1/inventory/refresh", post(refresh_inventory))
        .route("/api/v1/hardware", get(inventory))
        .route("/api/v1/interfaces", get(interfaces))
        .route("/api/v1/thermal", get(thermal))
        .route("/api/v1/integrations", get(integrations))
        .route("/api/v1/integrations/fleet/inventory", get(fleet_inventory))
        .route("/api/v1/plugins", get(list_plugins))
        .route("/api/v1/plugins/{name}/sample", post(sample_plugin))
        .route("/api/v1/sensors", get(sensors))
        .route("/api/v1/sensors/{sensor_id}", get(sensor))
        .route("/api/v1/events", get(events))
        .route("/api/v1/events/recent", get(recent_events))
        .route("/api/v1/doctor", get(doctor))
        .route("/metrics", get(metrics))
        .fallback_service(ServeDir::new(dashboard_dir).append_index_html_on_directories(true))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let current = state.inventory_snapshot().await;
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "serial": current.device.serial,
        "uptime_seconds": current.system.uptime_seconds,
        "inventory_generation": state.status().inventory_generation
    }))
}

async fn status(State(state): State<Arc<AppState>>) -> Json<crate::model::AgentStatus> {
    Json(state.status())
}

async fn inventory(State(state): State<Arc<AppState>>) -> Json<crate::model::Inventory> {
    Json(state.inventory_snapshot().await)
}

async fn refresh_inventory(State(state): State<Arc<AppState>>) -> Json<crate::model::Inventory> {
    let current = hardware::collect_inventory(&state.config).await;
    state.update_inventory(current.clone()).await;
    Json(current)
}

async fn interfaces(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let current = state.inventory_snapshot().await;
    Json(serde_json::json!({
        "network": current.network,
        "buses": current.buses,
        "usb": current.usb
    }))
}

async fn thermal(State(state): State<Arc<AppState>>) -> Json<Vec<crate::model::ThermalZone>> {
    Json(state.inventory_snapshot().await.thermal)
}

async fn integrations(State(state): State<Arc<AppState>>) -> Json<IntegrationStatus> {
    Json(IntegrationStatus {
        nodra_enabled: state.config.nodra.enabled,
        nodra_connected: state.nodra_connected(),
        fleet_enabled: state.config.fleet.enabled,
        fleet_projection_ready: state.config.fleet.enabled
            && state.config.fleet.mode == "projection",
    })
}

async fn fleet_inventory(
    State(state): State<Arc<AppState>>,
) -> Json<crate::integrations::fleet::FleetInventoryProjection> {
    let inventory = state.inventory_snapshot().await;
    Json(crate::integrations::fleet::project(&inventory))
}

async fn list_plugins(State(state): State<Arc<AppState>>) -> Json<Vec<plugins::PluginStatus>> {
    Json(plugins::statuses(&state.config))
}

async fn sample_plugin(Path(name): Path<String>, State(state): State<Arc<AppState>>) -> Response {
    let list = plugins::discover(&state.config);
    let Some(plugin) = list.iter().find(|plugin| plugin.name == name) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"plugin not found"})),
        )
            .into_response();
    };
    let sample = plugins::sample(&state.config, plugin).await;
    let status = if sample.ok {
        StatusCode::OK
    } else {
        StatusCode::BAD_GATEWAY
    };
    state.record_sample(sample.clone()).await;
    (status, Json(sample)).into_response()
}

async fn sensors(State(state): State<Arc<AppState>>) -> Json<Vec<crate::model::SensorSample>> {
    Json(state.latest_samples().await)
}

async fn sensor(Path(sensor_id): Path<String>, State(state): State<Arc<AppState>>) -> Response {
    match state.latest_sample(&sensor_id).await {
        Some(sample) => (StatusCode::OK, Json(sample)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"sensor has no sample yet"})),
        )
            .into_response(),
    }
}

async fn recent_events(State(state): State<Arc<AppState>>) -> Json<Vec<crate::model::AgentEvent>> {
    Json(state.recent_events())
}

async fn events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let stream = BroadcastStream::new(state.subscribe_events()).filter_map(|message| {
        let Ok(event) = message else {
            return None;
        };
        let data = serde_json::to_string(&event).unwrap_or_else(|_| "{}".into());
        Some(Ok(SseEvent::default().id(event.id.to_string()).data(data)))
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

async fn doctor(State(state): State<Arc<AppState>>) -> Json<crate::model::DoctorReport> {
    Json(hardware::doctor(&state.config).await)
}

async fn metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let current = state.inventory_snapshot().await;
    let status = state.status();
    let samples = state.latest_samples().await;
    let temp = current
        .thermal
        .iter()
        .filter_map(|zone| zone.celsius)
        .reduce(f64::max)
        .unwrap_or(f64::NAN);
    let sensor_ok = samples.iter().filter(|sample| sample.ok).count();
    let body = format!(
        "# HELP zyvor_device_agent_up Whether the device agent is serving requests.\n\
# TYPE zyvor_device_agent_up gauge\n\
zyvor_device_agent_up 1\n\
# HELP zyvor_device_agent_inventory_generation Monotonic inventory change generation.\n\
# TYPE zyvor_device_agent_inventory_generation counter\n\
zyvor_device_agent_inventory_generation {}\n\
# HELP zyvor_device_agent_temperature_celsius Highest available Linux thermal-zone temperature.\n\
# TYPE zyvor_device_agent_temperature_celsius gauge\n\
zyvor_device_agent_temperature_celsius {temp}\n\
# HELP zyvor_device_agent_network_interfaces Number of Linux network interfaces.\n\
# TYPE zyvor_device_agent_network_interfaces gauge\n\
zyvor_device_agent_network_interfaces {}\n\
# HELP zyvor_device_agent_can_interfaces Number of CAN interfaces.\n\
# TYPE zyvor_device_agent_can_interfaces gauge\n\
zyvor_device_agent_can_interfaces {}\n\
# HELP zyvor_device_agent_i2c_buses Number of I2C device nodes.\n\
# TYPE zyvor_device_agent_i2c_buses gauge\n\
zyvor_device_agent_i2c_buses {}\n\
# HELP zyvor_device_agent_sensor_samples_total Total sensor samples executed.\n\
# TYPE zyvor_device_agent_sensor_samples_total counter\n\
zyvor_device_agent_sensor_samples_total {}\n\
# HELP zyvor_device_agent_sensor_sample_failures_total Failed sensor samples.\n\
# TYPE zyvor_device_agent_sensor_sample_failures_total counter\n\
zyvor_device_agent_sensor_sample_failures_total {}\n\
# HELP zyvor_device_agent_sensors_ok Sensors with a latest successful sample.\n\
# TYPE zyvor_device_agent_sensors_ok gauge\n\
zyvor_device_agent_sensors_ok {sensor_ok}\n\
# HELP zyvor_device_agent_nodra_connected Nodra MQTT connection state.\n\
# TYPE zyvor_device_agent_nodra_connected gauge\n\
zyvor_device_agent_nodra_connected {}\n\
# HELP zyvor_device_agent_fleet_projection_ready Fleet-compatible local inventory projection is enabled.\n\
# TYPE zyvor_device_agent_fleet_projection_ready gauge\n\
zyvor_device_agent_fleet_projection_ready {}\n",
        status.inventory_generation,
        current.network.len(),
        current.buses.can.len(),
        current.buses.i2c.len(),
        status.sensor_samples_total,
        status.sensor_sample_failures,
        u8::from(state.nodra_connected()),
        u8::from(state.config.fleet.enabled && state.config.fleet.mode == "projection"),
    );
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], body)
}
