// SPDX-License-Identifier: Apache-2.0
//
// End-to-end HTTP-level tests for the camera routes, driven the same way
// tests/api_auth.rs drives the auth middleware: build the real Router and
// exercise it via tower::ServiceExt::oneshot. None of this needs a real
// camera or the `camera` feature - `record_camera_frame`/`is_configured_camera`
// are plain AppState methods, and a configured-but-not-yet-capturing camera
// id already exercises the 503 path with zero capture-thread involvement.

use std::sync::Arc;

use http_body_util::BodyExt;
use tower::ServiceExt;
use zyvor_device_agent::{
    api,
    config::{CameraDeviceConfig, Config},
    model::{CameraFrame, DeviceIdentity, Inventory, SystemInfo},
    state::AppState,
};

fn empty_inventory() -> Inventory {
    Inventory {
        device: DeviceIdentity {
            serial: "ZY-TEST".into(),
            vendor: "test".into(),
            model: "test".into(),
            hostname: "test".into(),
            machine_id: "test".into(),
        },
        system: SystemInfo {
            arch: "test".into(),
            kernel: "test".into(),
            os: "test".into(),
            cpu_model: "test".into(),
            cpu_cores: 1,
            memory_bytes: 0,
            storage_bytes: None,
            uptime_seconds: 0,
        },
        network: vec![],
        buses: Default::default(),
        industrial: Default::default(),
        usb: vec![],
        thermal: vec![],
        capabilities: vec![],
    }
}

fn config_with_camera(id: &str) -> Config {
    let mut config = Config::default();
    config.camera.devices.push(CameraDeviceConfig {
        id: id.to_string(),
        path: "/dev/video0".to_string(),
        enabled: true,
        ..Default::default()
    });
    config
}

async fn get(router: axum::Router, uri: &str) -> (axum::http::StatusCode, serde_json::Value) {
    let request = axum::http::Request::builder()
        .method("GET")
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, body)
}

#[tokio::test]
async fn unconfigured_camera_snapshot_is_404() {
    let state = Arc::new(AppState::new(Config::default(), empty_inventory()));
    let router = api::router(state);
    let (status, _) = get(router, "/api/v1/camera/unknown/snapshot").await;
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unconfigured_camera_stream_is_404() {
    let state = Arc::new(AppState::new(Config::default(), empty_inventory()));
    let router = api::router(state);
    let (status, _) = get(router, "/api/v1/camera/unknown/stream").await;
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn configured_camera_with_no_frame_yet_snapshot_is_503() {
    let state = Arc::new(AppState::new(
        config_with_camera("front-dock"),
        empty_inventory(),
    ));
    let router = api::router(state);
    let (status, _) = get(router, "/api/v1/camera/front-dock/snapshot").await;
    assert_eq!(status, axum::http::StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn camera_with_a_frame_snapshot_returns_jpeg_bytes() {
    let state = Arc::new(AppState::new(
        config_with_camera("front-dock"),
        empty_inventory(),
    ));
    state.record_camera_frame(
        "front-dock",
        CameraFrame {
            camera_id: "front-dock".into(),
            sequence: 0,
            captured_at_unix_ms: 0,
            width: 4,
            height: 2,
            content_type: "image/jpeg",
            jpeg: vec![0xFF, 0xD8, 0xFF, 0xD9],
        },
    );
    let router = api::router(state);
    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/api/v1/camera/front-dock/snapshot")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .unwrap(),
        "image/jpeg"
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], &[0xFF, 0xD8, 0xFF, 0xD9]);
}

#[tokio::test]
async fn camera_list_reports_configured_camera() {
    let state = Arc::new(AppState::new(
        config_with_camera("front-dock"),
        empty_inventory(),
    ));
    let router = api::router(state);
    let (status, body) = get(router, "/api/v1/camera").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let entries = body.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["id"], "front-dock");
    assert_eq!(entries[0]["capturing"], false);
}

#[tokio::test]
async fn configured_camera_stream_connects_even_with_no_frames_yet() {
    let state = Arc::new(AppState::new(
        config_with_camera("front-dock"),
        empty_inventory(),
    ));
    let router = api::router(state);
    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/api/v1/camera/front-dock/stream")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert!(response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("multipart/x-mixed-replace"));
}
