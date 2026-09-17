// SPDX-License-Identifier: Apache-2.0
//
// HTTP-level coverage for the edge-ai scaffold route. Requires
// `--features edge-ai` (same pattern as optional-feature CI legs).

#![cfg(feature = "edge-ai")]

use std::sync::Arc;

use http_body_util::BodyExt;
use tower::ServiceExt;
use zyvor_device_agent::{
    api,
    config::Config,
    model::{DeviceIdentity, Inventory, SystemInfo},
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

#[tokio::test]
async fn inference_events_returns_501_not_configured() {
    let state = Arc::new(AppState::new(Config::default(), empty_inventory()));
    let router = api::router(state);
    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/api/v1/inference/events")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NOT_IMPLEMENTED);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"], "not-configured");
    assert_eq!(body["configured"], false);
    assert_eq!(body["contract_version"], 1);
}
