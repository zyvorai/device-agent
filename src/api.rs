// SPDX-License-Identifier: Apache-2.0

use std::{convert::Infallible, fmt::Write as _, sync::Arc, time::Duration};

use axum::{
    body::Body,
    extract::{Path, Query, State},
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

use crate::{
    auth, bundle, diagnostics, hardware, model::IntegrationStatus, passport, plugins, privsep,
    recorder, remediation, state::AppState,
};

pub fn router(state: Arc<AppState>) -> Router {
    // Captured once at router-build time: ServeDir is wired to this path for the
    // life of the process, so changing server.dashboard_dir needs a restart.
    let dashboard_dir = state.config.load().server.dashboard_dir.clone();
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
        .route("/api/v1/camera", get(camera_list))
        .route("/api/v1/camera/{id}/snapshot", get(camera_snapshot))
        .route("/api/v1/camera/{id}/stream", get(camera_stream))
        .merge(edge_ai_routes())
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
        .route("/api/v1/stream-tickets", post(issue_stream_ticket))
        .route("/api/v1/passport", get(passport))
        .route("/api/v1/passport/verify", post(passport_verify))
        .route("/api/v1/recorder", get(recorder_query))
        .route("/api/v1/diagnostics/findings", get(diagnostic_findings))
        .route("/api/v1/support-bundle/preview", post(support_preview))
        .route("/api/v1/support-bundle", post(support_download))
        .route("/api/v1/remediation", post(remediate))
        .route("/api/v1/commissioning", get(commissioning_status))
        .route("/metrics", get(metrics))
        .fallback_service(ServeDir::new(dashboard_dir).append_index_html_on_directories(true))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::dispatch,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(auth::redact_stream_ticket))
        .with_state(state)
}

/// `--features edge-ai` only: placeholder inference contract. Empty merge
/// otherwise so a plain `cargo build` never grows new routes.
fn edge_ai_routes() -> Router<Arc<AppState>> {
    #[cfg(feature = "edge-ai")]
    {
        Router::new().route(
            "/api/v1/inference/events",
            get(crate::edge_ai::inference_events),
        )
    }
    #[cfg(not(feature = "edge-ai"))]
    {
        Router::new()
    }
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
    let refresh_interval_ms = state.config.load().device.inventory_refresh_seconds.max(1) * 1000;
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
    let current = hardware::collect_inventory(&state.config.load()).await;
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

async fn camera_list(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<crate::model::CameraCaptureStatus>> {
    Json(state.camera_capture_statuses())
}

async fn camera_snapshot(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Response {
    if !state.is_configured_camera(&id) {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "camera not configured"})),
        )
            .into_response();
    }
    match state.latest_camera_frame(&id) {
        Some(frame) => (
            [(header::CONTENT_TYPE, frame.content_type)],
            frame.jpeg.clone(),
        )
            .into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "no frame captured yet"})),
        )
            .into_response(),
    }
}

/// `multipart/x-mixed-replace` — the same shape a classic IP camera serves,
/// which every browser already renders natively inside a plain `<img>` tag
/// with no WebRTC/signaling needed. Capture runs exactly once regardless of
/// how many viewers connect (`AppState::record_camera_frame` broadcasts to
/// all subscribers); a lagging viewer skips forward rather than tearing
/// down the connection, same as the CAN/events SSE handlers above silently
/// dropping `Err` broadcast results.
async fn camera_stream(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Response {
    const BOUNDARY: &str = "zyvorframe";

    let Some(receiver) = state.subscribe_camera_frames(&id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "camera not configured"})),
        )
            .into_response();
    };

    let max_clients = state
        .config
        .load()
        .camera
        .devices
        .iter()
        .find(|device| device.id == id)
        .map(|device| device.max_stream_clients)
        .unwrap_or(0);
    // 0 = unlimited, matching this config's existing "0/unset = no limit"
    // idiom. Checked after subscribing (so the count already includes this
    // connection) - a brief overshoot on the rejected connection's way back
    // out is fine for a soft cap like this.
    if max_clients > 0 && state.camera_stream_subscriber_count(&id) > max_clients as usize {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({"error": "too many concurrent stream viewers"})),
        )
            .into_response();
    }

    let stream = BroadcastStream::new(receiver).filter_map(move |message| {
        let Ok(frame) = message else {
            // Lagged: skip forward and keep streaming rather than ending it.
            return None;
        };
        let mut chunk = format!(
            "--{BOUNDARY}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n",
            frame.content_type,
            frame.jpeg.len()
        )
        .into_bytes();
        chunk.extend_from_slice(&frame.jpeg);
        chunk.extend_from_slice(b"\r\n");
        Some(Ok::<_, Infallible>(chunk))
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/x-mixed-replace; boundary={BOUNDARY}"),
        )
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn thermal(State(state): State<Arc<AppState>>) -> Json<Vec<crate::model::ThermalZone>> {
    Json(state.inventory_snapshot().await.thermal)
}

async fn integrations(State(state): State<Arc<AppState>>) -> Json<IntegrationStatus> {
    let config = state.config.load();
    Json(IntegrationStatus {
        nodra_enabled: config.nodra.enabled,
        nodra_connected: state.nodra_connected(),
        fleet_enabled: config.fleet.enabled,
        fleet_projection_ready: config.fleet.enabled && config.fleet.mode == "projection",
    })
}

async fn fleet_inventory(
    State(state): State<Arc<AppState>>,
) -> Json<crate::integrations::fleet::FleetInventoryProjection> {
    let inventory = state.inventory_snapshot().await;
    Json(crate::integrations::fleet::project_signed(
        &inventory,
        &state.config.load(),
    ))
}

async fn list_plugins(State(state): State<Arc<AppState>>) -> Json<Vec<plugins::PluginStatus>> {
    Json(plugins::statuses(&state.config.load()))
}

async fn sample_plugin(Path(name): Path<String>, State(state): State<Arc<AppState>>) -> Response {
    let config = state.config.load_full();
    let list = plugins::discover(&config);
    let Some(plugin) = list.iter().find(|plugin| plugin.name == name) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"plugin not found"})),
        )
            .into_response();
    };
    let sample = plugins::sample(&config, plugin).await;
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
    let cfg = state.config.load_full();
    if privsep::direct_bus_access(&cfg.privsep) {
        return Json(hardware::doctor(&cfg).await);
    }
    match privsep::fetch_doctor(&cfg).await {
        Ok(report) => Json(report),
        Err(error) => Json(crate::model::DoctorReport {
            ok: false,
            checks: vec![crate::model::DoctorCheck {
                name: "bus-helper".into(),
                ok: false,
                detail: error.to_string(),
            }],
        }),
    }
}

#[derive(serde::Deserialize)]
struct TicketRequest {
    path_prefix: String,
}

async fn issue_stream_ticket(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TicketRequest>,
) -> Response {
    let ttl = state.config.load().auth.stream_ticket_ttl_seconds;
    match state.stream_tickets().issue(&request.path_prefix, ttl) {
        Ok(ticket) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "ticket": ticket,
                "pathPrefix": request.path_prefix,
                "ttlSeconds": ttl,
            })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

async fn passport(State(state): State<Arc<AppState>>) -> Json<passport::Passport> {
    let cfg = state.config.load_full();
    let inventory = state.inventory_snapshot().await;
    let report = hardware::doctor(&cfg).await;
    Json(passport::build(&cfg, &inventory, Some(&report)))
}

async fn passport_verify(Json(document): Json<passport::Passport>) -> Response {
    match passport::verify_document(&document) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"ok": false, "error": error.to_string()})),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
struct RecorderQuery {
    #[serde(default)]
    since: String,
}

async fn recorder_query(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RecorderQuery>,
) -> Response {
    let cfg = state.config.load_full();
    let now = crate::state::now_unix_ms();
    let since = if query.since.is_empty() {
        now.saturating_sub(15 * 60 * 1000)
    } else {
        match recorder::parse_since(&query.since, now) {
            Ok(value) => value,
            Err(error) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": error.to_string()})),
                )
                    .into_response();
            }
        }
    };
    match recorder::query(&recorder::directory(&cfg), since, u64::MAX) {
        Ok(records) => Json(records).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

async fn diagnostic_findings(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<diagnostics::Finding>> {
    let inventory = state.inventory_snapshot().await;
    let events = state.recent_events();
    let nodra = Some(state.nodra_connected());
    Json(diagnostics::findings(
        &inventory,
        &events,
        None,
        crate::state::now_unix_ms() as i64,
        nodra,
    ))
}

#[derive(serde::Deserialize)]
struct BundleRequest {
    #[serde(default = "default_since")]
    since: String,
    #[serde(default = "default_true")]
    redact: bool,
}

fn default_since() -> String {
    "2h".into()
}

fn default_true() -> bool {
    true
}

async fn support_preview(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BundleRequest>,
) -> Response {
    let cfg = state.config.load_full();
    let inventory = state.inventory_snapshot().await;
    let report = hardware::doctor(&cfg).await;
    let since = recorder::parse_since(&request.since, crate::state::now_unix_ms()).unwrap_or(0);
    match bundle::preview(&cfg, &inventory, &report, since, request.redact) {
        Ok(manifest) => Json(manifest).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

async fn support_download(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BundleRequest>,
) -> Response {
    let cfg = state.config.load_full();
    let inventory = state.inventory_snapshot().await;
    let report = hardware::doctor(&cfg).await;
    let since = recorder::parse_since(&request.since, crate::state::now_unix_ms()).unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
        "zyvor-support-{}-{}.tar.zst",
        std::process::id(),
        crate::state::now_unix_ms()
    ));
    match bundle::write_archive(&cfg, &inventory, &report, since, request.redact, &path) {
        Ok(_) => match std::fs::read(&path) {
            Ok(bytes) => {
                let _ = std::fs::remove_file(&path);
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "application/zstd")],
                    bytes,
                )
                    .into_response()
            }
            Err(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": error.to_string()})),
            )
                .into_response(),
        },
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

async fn remediate(
    State(state): State<Arc<AppState>>,
    Json(command): Json<remediation::Command>,
) -> Response {
    let cfg = state.config.load_full();
    let mut replay = remediation::load_replay(&cfg);
    match remediation::execute(&cfg, &command, crate::state::now_unix_ms(), &mut replay) {
        Ok(result) => {
            let _ = remediation::store_replay(&cfg, &replay);
            state.emit_event(
                "remediation.completed",
                serde_json::json!({"action": result.action, "ok": result.ok, "detail": result.detail}),
            );
            Json(result).into_response()
        }
        Err(error) => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

async fn commissioning_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let cfg = state.config.load();
    let identity_present = std::path::Path::new(&cfg.auth.mtls.cert_file).exists();
    let loopback = crate::config::listen_is_loopback(&cfg.server.listen).unwrap_or(false);
    Json(serde_json::json!({
        "authMode": cfg.auth.mode,
        "listenLoopback": loopback,
        "enrollmentEnabled": cfg.enrollment.enabled,
        "identityPresent": identity_present,
        "passportReady": true,
        "needsWizard": cfg.auth.mode == "none" && !cfg.enrollment.enabled,
    }))
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
        u8::from({
            let config = state.config.load();
            config.fleet.enabled && config.fleet.mode == "projection"
        }),
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
