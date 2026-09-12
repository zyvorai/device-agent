// SPDX-License-Identifier: Apache-2.0

//! API auth. `mode = "none"` (default, backward compatible) enforces nothing. `mode =
//! "bearer"` requires `Authorization: Bearer <token>` on every `/api/*` route and on
//! `/metrics`, except `auth.exempt_paths`. The daemon never generates or stores the raw
//! token — only its SHA-256 hash, loaded once at startup from `auth.bearer.token_hash_file`
//! (see scripts/deploy-remote.sh for how that file gets populated).
//!
//! Everything outside `/api/*`/`/metrics` — the bundled dashboard's static shell
//! (`index.html`, JS, CSS, served via `ServeDir`) — is unconditionally exempt,
//! unauthenticated even in `mode = "bearer"`. That shell contains no device data; it is
//! the same JS bundle regardless of auth state, and it must load before it can show its
//! own token-entry UI.
//!
//! Browsers' `EventSource` cannot set custom headers, so the two SSE routes
//! additionally accept the token as a `?token=` query parameter. This is
//! deliberately narrower than the header path: it is only checked for
//! [`SSE_QUERY_TOKEN_PATHS`], never as a general alternative to the
//! `Authorization` header, since a query-string token can land in access logs
//! and browser history.

pub mod bearer;
pub mod uds;

use std::sync::Arc;

use axum::{
    extract::{Query, Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::state::AppState;

/// Routes that accept a `?token=` query-string fallback, in addition to the
/// `Authorization` header, because they are consumed via `EventSource`.
const SSE_QUERY_TOKEN_PATHS: &[&str] = &["/api/v1/events", "/api/v1/can/frames/stream"];

#[derive(Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

pub async fn dispatch(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();

    // Every data-bearing route lives under /api/ or is exactly /metrics (see
    // api::router()); anything else is the bundled dashboard's static shell
    // (index.html, JS, CSS), served unauthenticated so the app can load far
    // enough to show its own token-entry UI. This is standard for SPAs behind
    // token auth: the shell is public, the API it calls is not.
    if is_static_shell_path(path) {
        return next.run(request).await;
    }

    let config = state.config.load();
    if config.auth.exempt_paths.iter().any(|exempt| exempt == path) {
        return next.run(request).await;
    }

    match config.auth.mode.as_str() {
        "none" => next.run(request).await,
        "bearer" => match check_bearer(&state, &request) {
            Ok(()) => next.run(request).await,
            Err(message) => unauthorized(message),
        },
        other => {
            tracing::error!(
                mode = other,
                "unsupported auth.mode configured; denying request"
            );
            unauthorized("unsupported auth mode configured")
        }
    }
}

fn is_static_shell_path(path: &str) -> bool {
    !path.starts_with("/api/") && path != "/metrics"
}

fn check_bearer(state: &AppState, request: &Request) -> Result<(), &'static str> {
    let Some(expected) = state.bearer_token_hash() else {
        return Err("bearer auth enabled but no token is configured");
    };

    if let Some(header_value) = request.headers().get(header::AUTHORIZATION) {
        let Ok(value) = header_value.to_str() else {
            return Err("invalid Authorization header");
        };
        let Some(token) = value.strip_prefix("Bearer ") else {
            return Err("Authorization header must use the Bearer scheme");
        };
        return if bearer::verify(token, &expected) {
            Ok(())
        } else {
            Err("invalid bearer token")
        };
    }

    if SSE_QUERY_TOKEN_PATHS.contains(&request.uri().path()) {
        if let Some(token) = query_token(request) {
            return if bearer::verify(&token, &expected) {
                Ok(())
            } else {
                Err("invalid bearer token")
            };
        }
    }

    Err("missing Authorization header")
}

fn query_token(request: &Request) -> Option<String> {
    request.uri().query()?;
    let parsed: TokenQuery = Query::try_from_uri(request.uri()).ok()?.0;
    parsed.token.filter(|token| !token.is_empty())
}

fn unauthorized(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Config,
        model::{DeviceIdentity, Inventory, SystemInfo},
    };
    use sha2::{Digest, Sha256};

    fn request_with_query(path_and_query: &str) -> Request {
        Request::builder()
            .uri(path_and_query)
            .body(axum::body::Body::empty())
            .unwrap()
    }

    fn request_with_auth_header(value: Option<&str>) -> Request {
        let mut builder = Request::builder().uri("/api/v1/inventory");
        if let Some(value) = value {
            builder = builder.header(header::AUTHORIZATION, value);
        }
        builder.body(axum::body::Body::empty()).unwrap()
    }

    fn empty_inventory() -> Inventory {
        Inventory {
            device: DeviceIdentity {
                serial: "test".into(),
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

    /// Builds an AppState in `auth.mode = "bearer"` with a real hash file on disk,
    /// exercising the same `AppState::new` -> `bearer::load_token_hash` path used
    /// in production, not a test-only backdoor.
    struct BearerState {
        state: AppState,
        _hash_file: TempHashFile,
    }
    struct TempHashFile(std::path::PathBuf);
    impl Drop for TempHashFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn state_with_bearer_token(token: &str) -> BearerState {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);

        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        let hash: [u8; 32] = hasher.finalize().into();
        let hex_hash = hash.iter().map(|b| format!("{b:02x}")).collect::<String>();

        let path = std::env::temp_dir().join(format!(
            "zyvor-auth-test-bearer-{}-{id}.sha256",
            std::process::id()
        ));
        std::fs::write(&path, &hex_hash).unwrap();

        let mut cfg = Config::default();
        cfg.auth.mode = "bearer".into();
        cfg.auth.bearer.token_hash_file = path.to_string_lossy().into_owned();
        let state = AppState::new(cfg, empty_inventory());
        assert!(
            state.bearer_token_hash().is_some(),
            "test setup: hash file should have loaded"
        );
        BearerState {
            state,
            _hash_file: TempHashFile(path),
        }
    }

    #[test]
    fn query_token_reads_token_param() {
        let request = request_with_query("/api/v1/events?token=abc123");
        assert_eq!(query_token(&request).as_deref(), Some("abc123"));
    }

    #[test]
    fn query_token_ignores_other_params() {
        let request = request_with_query("/api/v1/events?other=1");
        assert_eq!(query_token(&request), None);
    }

    #[test]
    fn query_token_treats_empty_token_as_absent() {
        let request = request_with_query("/api/v1/events?token=");
        assert_eq!(query_token(&request), None);
    }

    #[test]
    fn query_token_absent_with_no_query_string() {
        let request = request_with_query("/api/v1/events");
        assert_eq!(query_token(&request), None);
    }

    #[test]
    fn static_shell_paths_are_exempt() {
        assert!(is_static_shell_path("/"));
        assert!(is_static_shell_path("/index.html"));
        assert!(is_static_shell_path("/assets/index-abc123.js"));
        assert!(is_static_shell_path("/assets/index-abc123.css"));
    }

    #[test]
    fn api_and_metrics_paths_are_not_exempt() {
        assert!(!is_static_shell_path("/api/v1/inventory"));
        assert!(!is_static_shell_path("/api/v1/health"));
        assert!(!is_static_shell_path("/metrics"));
    }

    #[test]
    fn sse_query_token_paths_are_exact() {
        assert!(SSE_QUERY_TOKEN_PATHS.contains(&"/api/v1/events"));
        assert!(SSE_QUERY_TOKEN_PATHS.contains(&"/api/v1/can/frames/stream"));
        assert!(!SSE_QUERY_TOKEN_PATHS.contains(&"/api/v1/inventory"));
    }

    #[test]
    fn check_bearer_rejects_missing_header() {
        let bearer = state_with_bearer_token("secret");
        let request = request_with_auth_header(None);
        assert_eq!(
            check_bearer(&bearer.state, &request),
            Err("missing Authorization header")
        );
    }

    #[test]
    fn check_bearer_rejects_wrong_token() {
        let bearer = state_with_bearer_token("secret");
        let request = request_with_auth_header(Some("Bearer wrong-token"));
        assert_eq!(
            check_bearer(&bearer.state, &request),
            Err("invalid bearer token")
        );
    }

    #[test]
    fn check_bearer_rejects_non_bearer_scheme() {
        let bearer = state_with_bearer_token("secret");
        let request = request_with_auth_header(Some("Basic dXNlcjpwYXNz"));
        assert_eq!(
            check_bearer(&bearer.state, &request),
            Err("Authorization header must use the Bearer scheme")
        );
    }

    #[test]
    fn check_bearer_accepts_correct_token() {
        let bearer = state_with_bearer_token("secret");
        let request = request_with_auth_header(Some("Bearer secret"));
        assert_eq!(check_bearer(&bearer.state, &request), Ok(()));
    }

    #[test]
    fn check_bearer_fails_closed_with_no_hash_configured() {
        // mode = "bearer" but the hash file doesn't exist / didn't load.
        let mut cfg = Config::default();
        cfg.auth.mode = "bearer".into();
        cfg.auth.bearer.token_hash_file = "/nonexistent/zyvor-test-bearer.sha256".into();
        let state = AppState::new(cfg, empty_inventory());
        assert!(state.bearer_token_hash().is_none());
        let request = request_with_auth_header(Some("Bearer anything"));
        assert_eq!(
            check_bearer(&state, &request),
            Err("bearer auth enabled but no token is configured")
        );
    }
}
