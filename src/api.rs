// SPDX-License-Identifier: Apache-2.0

use std::{convert::Infallible, fmt::Write as _, sync::Arc, time::Duration};

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    middleware,
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};
use tower_http::{services::ServeDir, trace::TraceLayer};

use crate::{auth, hardware, model::IntegrationStatus, plugins, state::AppState};

pub fn router(state: Arc<AppState>) -> Router {
    let dashboard_dir = state.config.server.dashboard_dir.clone();
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/ready", get(ready))
        .route("/api/v1/status", get(status))
        .route("/api/v1/inventory", get(inventory))
        .route("/api/v1/inventory/refresh", post(refresh_inventory))
        .route("/api/v1/hardware", get(inventory))
        .route("/api/v1/interfaces", get(interfaces))
        .route("/api/v1/industrial", get(industrial))
        .route("/api/v1/industrial/can", get(industrial_can))
        .route("/api/v1/industrial/serial", get(industrial_serial))
        .route("/api/v1/can/capture", get(can_capture_status))
        .route("/api/v1/can/frames/recent", get(can_frames_recent))
        .route("/api/v1/can/frames/stream", get(can_frames_stream))
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
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::dispatch,
        ))
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

/// Liveness (`/api/v1/health`) only proves the process is up and answering.
/// Readiness additionally checks that the background inventory refresh loop
/// (`spawn_inventory_refresh` in main.rs) is still ticking, without doing any
/// real hardware probing itself (that's `/api/v1/doctor`, which stays a
/// separate, deliberately heavier diagnostic endpoint). A stale timestamp
/// here means the refresh task has stalled or panicked.
async fn ready(State(state): State<Arc<AppState>>) -> Response {
    let status = state.status();
    let refresh_interval_ms = state.config.device.inventory_refresh_seconds.max(1) * 1000;
    // Allow generous slack over the configured interval before calling the
    // agent unready - a single slow tick under load must not flip probes.
    let staleness_budget_ms = (refresh_interval_ms.saturating_mul(3)).max(30_000);
    let age_ms = crate::state::now_unix_ms().saturating_sub(status.last_inventory_refresh_unix_ms);
    let ready = age_ms <= staleness_budget_ms;
    let body = serde_json::json!({
        "ready": ready,
        "inventory_generation": status.inventory_generation,
        "inventory_age_ms": age_ms,
        "staleness_budget_ms": staleness_budget_ms,
    });
    let code = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (code, Json(body)).into_response()
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
        "industrial": current.industrial,
        "usb": current.usb
    }))
}

async fn industrial(State(state): State<Arc<AppState>>) -> Json<crate::model::IndustrialInventory> {
    Json(state.inventory_snapshot().await.industrial)
}

async fn industrial_can(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<crate::model::CanInterfaceInfo>> {
    Json(state.inventory_snapshot().await.industrial.can)
}

async fn industrial_serial(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<crate::model::SerialPortInfo>> {
    Json(state.inventory_snapshot().await.industrial.serial)
}

async fn can_capture_status(
    State(state): State<Arc<AppState>>,
) -> Json<crate::model::CanCaptureStatus> {
    Json(state.can_capture_status())
}

async fn can_frames_recent(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<crate::model::CanFrame>> {
    Json(state.recent_can_frames())
}

async fn can_frames_stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let stream = BroadcastStream::new(state.subscribe_can_frames()).filter_map(|message| {
        let Ok(frame) = message else {
            return None;
        };
        let data = serde_json::to_string(&frame).unwrap_or_else(|_| "{}".into());
        Some(Ok(SseEvent::default()
            .event("can.frame")
            .id(frame.sequence.to_string())
            .data(data)))
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
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
    let rs485_ports = current
        .industrial
        .serial
        .iter()
        .filter(|port| port.rs485.is_some())
        .count();

    let mut body = format!(
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
# HELP zyvor_device_agent_rs485_declared_ports Number of serial ports declared as RS485.\n\
# TYPE zyvor_device_agent_rs485_declared_ports gauge\n\
zyvor_device_agent_rs485_declared_ports {rs485_ports}\n\
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
# HELP zyvor_device_agent_can_capture_frames_total Read-only SocketCAN frames accepted by the capture path.\n\
# TYPE zyvor_device_agent_can_capture_frames_total counter\n\
zyvor_device_agent_can_capture_frames_total {}\n\
# HELP zyvor_device_agent_can_capture_dropped_total Frames dropped by the configured capture rate limit.\n\
# TYPE zyvor_device_agent_can_capture_dropped_total counter\n\
zyvor_device_agent_can_capture_dropped_total {}\n\
# HELP zyvor_device_agent_nodra_connected Nodra MQTT connection state.\n\
# TYPE zyvor_device_agent_nodra_connected gauge\n\
zyvor_device_agent_nodra_connected {}\n\
# HELP zyvor_device_agent_fleet_projection_ready Fleet-compatible local inventory projection is enabled.\n\
# TYPE zyvor_device_agent_fleet_projection_ready gauge\n\
zyvor_device_agent_fleet_projection_ready {}\n",
        status.inventory_generation,
        current.network.len(),
        current.industrial.can.len(),
        current.buses.i2c.len(),
        status.sensor_samples_total,
        status.sensor_sample_failures,
        status.can_capture_frames_total,
        status.can_capture_dropped_total,
        u8::from(state.nodra_connected()),
        u8::from(state.config.fleet.enabled && state.config.fleet.mode == "projection"),
    );

    body.push_str(
        "# HELP zyvor_device_agent_can_up Whether a CAN netdevice is operationally up.\n",
    );
    body.push_str("# TYPE zyvor_device_agent_can_up gauge\n");
    body.push_str(
        "# HELP zyvor_device_agent_can_bus_off Whether the CAN controller reports BUS-OFF.\n",
    );
    body.push_str("# TYPE zyvor_device_agent_can_bus_off gauge\n");
    body.push_str("# HELP zyvor_device_agent_can_bitrate_bits_per_second Configured CAN nominal bitrate when discoverable.\n");
    body.push_str("# TYPE zyvor_device_agent_can_bitrate_bits_per_second gauge\n");
    body.push_str(
        "# HELP zyvor_device_agent_can_rx_errors_total Linux netdevice CAN receive errors.\n",
    );
    body.push_str("# TYPE zyvor_device_agent_can_rx_errors_total counter\n");
    body.push_str(
        "# HELP zyvor_device_agent_can_tx_errors_total Linux netdevice CAN transmit errors.\n",
    );
    body.push_str("# TYPE zyvor_device_agent_can_tx_errors_total counter\n");

    for can in &current.industrial.can {
        let label = prometheus_label(&can.name);
        let bus_off = can
            .can_state
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("bus-off"));
        let _ = writeln!(
            body,
            "zyvor_device_agent_can_up{{interface=\"{label}\"}} {}",
            u8::from(can.operstate.eq_ignore_ascii_case("up"))
        );
        let _ = writeln!(
            body,
            "zyvor_device_agent_can_bus_off{{interface=\"{label}\"}} {}",
            u8::from(bus_off)
        );
        if let Some(bitrate) = can.bitrate {
            let _ = writeln!(
                body,
                "zyvor_device_agent_can_bitrate_bits_per_second{{interface=\"{label}\"}} {bitrate}"
            );
        }
        if let Some(errors) = can.rx_errors {
            let _ = writeln!(
                body,
                "zyvor_device_agent_can_rx_errors_total{{interface=\"{label}\"}} {errors}"
            );
        }
        if let Some(errors) = can.tx_errors {
            let _ = writeln!(
                body,
                "zyvor_device_agent_can_tx_errors_total{{interface=\"{label}\"}} {errors}"
            );
        }
    }

    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], body)
}

fn prometheus_label(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}
