// SPDX-License-Identifier: Apache-2.0
//
// End-to-end HTTP-level test: builds the real Router (auth::dispatch middleware
// included) and drives it via tower::ServiceExt::oneshot, proving the middleware
// wiring works together rather than each unit in isolation. Unit tests for the
// individual decision functions (check_bearer, is_static_shell_path, ...) live
// alongside the code in src/auth/mod.rs.

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

/// Writes a bearer token hash to a unique temp file and returns a Config wired
/// to use it in `auth.mode = "bearer"`, plus a guard that deletes the file on drop.
struct BearerFixture {
    config: Config,
    _hash_file: HashFileGuard,
}
struct HashFileGuard(std::path::PathBuf);
impl Drop for HashFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn bearer_config(token: &str) -> BearerFixture {
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);

    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let hash: [u8; 32] = hasher.finalize().into();
    let hex_hash = hash.iter().map(|b| format!("{b:02x}")).collect::<String>();

    let path = std::env::temp_dir().join(format!(
        "zyvor-api-auth-test-{}-{id}.sha256",
        std::process::id()
    ));
    std::fs::write(&path, &hex_hash).unwrap();

    let mut config = Config::default();
    config.auth.mode = "bearer".into();
    config.auth.bearer.token_hash_file = path.to_string_lossy().into_owned();
    BearerFixture {
        config,
        _hash_file: HashFileGuard(path),
    }
}

async fn status_for(
    router: axum::Router,
    method: &str,
    uri: &str,
    auth_header: Option<&str>,
) -> (axum::http::StatusCode, serde_json::Value) {
    let mut builder = axum::http::Request::builder().method(method).uri(uri);
    if let Some(value) = auth_header {
        builder = builder.header(axum::http::header::AUTHORIZATION, value);
    }
    let request = builder.body(axum::body::Body::empty()).unwrap();
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, body)
}

#[tokio::test]
async fn health_is_reachable_with_auth_mode_none() {
    let state = Arc::new(AppState::new(Config::default(), empty_inventory()));
    let router = api::router(state);
    let (status, _) = status_for(router, "GET", "/api/v1/health", None).await;
    assert_eq!(status, axum::http::StatusCode::OK);
}

#[tokio::test]
async fn inventory_is_reachable_with_auth_mode_none() {
    let state = Arc::new(AppState::new(Config::default(), empty_inventory()));
    let router = api::router(state);
    let (status, body) = status_for(router, "GET", "/api/v1/inventory", None).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["device"]["serial"], "ZY-TEST");
}

#[tokio::test]
async fn bearer_mode_rejects_inventory_without_token() {
    let fixture = bearer_config("secret-token");
    let state = Arc::new(AppState::new(fixture.config, empty_inventory()));
    let router = api::router(state);
    let (status, _) = status_for(router, "GET", "/api/v1/inventory", None).await;
    assert_eq!(status, axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn bearer_mode_rejects_inventory_with_wrong_token() {
    let fixture = bearer_config("secret-token");
    let state = Arc::new(AppState::new(fixture.config, empty_inventory()));
    let router = api::router(state);
    let (status, _) = status_for(
        router,
        "GET",
        "/api/v1/inventory",
        Some("Bearer wrong-token"),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn bearer_mode_accepts_inventory_with_correct_token() {
    let fixture = bearer_config("secret-token");
    let state = Arc::new(AppState::new(fixture.config, empty_inventory()));
    let router = api::router(state);
    let (status, body) = status_for(
        router,
        "GET",
        "/api/v1/inventory",
        Some("Bearer secret-token"),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["device"]["serial"], "ZY-TEST");
}

#[tokio::test]
async fn bearer_mode_still_allows_health_without_token() {
    let fixture = bearer_config("secret-token");
    let state = Arc::new(AppState::new(fixture.config, empty_inventory()));
    let router = api::router(state);
    let (status, _) = status_for(router, "GET", "/api/v1/health", None).await;
    assert_eq!(status, axum::http::StatusCode::OK);
}

#[tokio::test]
async fn bearer_mode_still_serves_the_dashboard_shell_without_token() {
    let fixture = bearer_config("secret-token");
    let state = Arc::new(AppState::new(fixture.config, empty_inventory()));
    let router = api::router(state);
    // dashboard_dir points nowhere in this test config, so ServeDir 404s - but
    // that 404 must come from ServeDir (no dashboard files installed here), not
    // from the auth middleware (401), proving the static-shell exemption applies
    // before any file lookup happens.
    let (status, _) = status_for(router, "GET", "/", None).await;
    assert_ne!(status, axum::http::StatusCode::UNAUTHORIZED);
}
