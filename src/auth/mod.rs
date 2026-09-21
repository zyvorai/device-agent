// SPDX-License-Identifier: Apache-2.0

//! API auth. `mode = "none"` (default, backward compatible) enforces nothing. `mode =
//! "bearer"` requires `Authorization: Bearer <token>` on every `/api/*` route and on
//! `/metrics`, except `auth.exempt_paths`. The daemon never generates or stores the raw
//! token — only its SHA-256 hash, loaded once at startup from `auth.bearer.token_hash_file`
//! (see scripts/deploy-remote.sh for how that file gets populated). `mode = "mtls"` moves
//! authentication to the TLS layer itself (see [`mtls`]): the listener requires a client
//! certificate signed by `auth.mtls.client_ca_file` before the handshake even completes,
//! so this middleware has nothing left to check for that mode. See [`enroll`] for how a
//! device obtains its own certificate in the first place.
//!
//! Everything outside `/api/*`/`/metrics` — the bundled dashboard's static shell
//! (`index.html`, JS, CSS, served via `ServeDir`) — is unconditionally exempt,
//! unauthenticated even in `mode = "bearer"`. That shell contains no device data; it is
//! the same JS bundle regardless of auth state, and it must load before it can show its
//! own token-entry UI.
//!
//! Browsers cannot set `Authorization` on `<img>` requests. Those camera
//! routes accept a short-lived stream ticket (`?ticket=`), minted by
//! `POST /api/v1/stream-tickets` under normal bearer or mTLS auth. The
//! long-lived bearer is never accepted from a query string. SSE uses
//! `fetch()` with the `Authorization` header, so it does not need a ticket,
//! but the same ticket path is allowed for it. A middleware strips `ticket`
//! from the URI before access logs run.

pub mod bearer;
pub mod enroll;
pub mod mtls;
pub mod tickets;
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

#[derive(Clone)]
pub struct PresentedStreamTicket(pub String);

#[derive(Deserialize)]
struct TicketQuery {
    ticket: Option<String>,
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
        // The TLS layer (see auth::mtls) already required and verified a
        // client certificate before this request could ever arrive here -
        // there is nothing left for application-level middleware to check.
        "mtls" => next.run(request).await,
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

    let path = request.uri().path();
    if tickets::is_stream_path(path) {
        if let Some(ticket) = presented_ticket(request) {
            return state.stream_tickets().verify(&ticket, path);
        }
    }

    Err("missing Authorization header")
}

fn presented_ticket(request: &Request) -> Option<String> {
    if let Some(PresentedStreamTicket(ticket)) = request.extensions().get() {
        if !ticket.is_empty() {
            return Some(ticket.clone());
        }
    }
    request.uri().query()?;
    let parsed: TicketQuery = Query::try_from_uri(request.uri()).ok()?.0;
    parsed.ticket.filter(|ticket| !ticket.is_empty())
}

/// Outermost middleware: move `?ticket=` into request extensions and drop it
/// from the URI so access logs do not record the credential.
pub async fn redact_stream_ticket(mut request: Request, next: Next) -> Response {
    if let Some(ticket) = take_ticket_query(&mut request) {
        request
            .extensions_mut()
            .insert(PresentedStreamTicket(ticket));
    }
    next.run(request).await
}

fn take_ticket_query(request: &mut Request) -> Option<String> {
    let query = request.uri().query()?.to_string();
    let mut ticket = None;
    let mut kept = Vec::new();
    for part in query.split('&') {
        if let Some(value) = part.strip_prefix("ticket=") {
            if ticket.is_none() && !value.is_empty() {
                ticket = Some(urlencoding_decode(value).unwrap_or_else(|| value.to_string()));
            }
        } else if !part.is_empty() {
            kept.push(part.to_string());
        }
    }
    ticket.as_ref()?;
    let path = request.uri().path().to_string();
    let rebuilt = if kept.is_empty() {
        path
    } else {
        format!("{path}?{}", kept.join("&"))
    };
    if let Ok(uri) = rebuilt.parse() {
        *request.uri_mut() = uri;
    }
    ticket
}

fn urlencoding_decode(value: &str) -> Option<String> {
    let mut out = Vec::new();
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_val(bytes[i + 1])?;
                let lo = hex_val(bytes[i + 2])?;
                out.push((hi << 4) | lo);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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
    fn query_bearer_is_not_accepted() {
        let bearer = state_with_bearer_token("secret");
        for path in [
            "/api/v1/events?token=secret",
            "/api/v1/camera/front-dock/snapshot?token=secret",
            "/api/v1/inventory?token=secret",
        ] {
            let request = request_with_query(path);
            assert_eq!(
                check_bearer(&bearer.state, &request),
                Err("missing Authorization header")
            );
        }
    }

    #[test]
    fn stream_ticket_authorizes_camera_but_not_inventory() {
        let bearer = state_with_bearer_token("secret");
        let ticket = bearer
            .state
            .stream_tickets()
            .issue("/api/v1/camera/front-dock/snapshot", 60)
            .unwrap();
        let ok = request_with_query(&format!(
            "/api/v1/camera/front-dock/snapshot?ticket={ticket}"
        ));
        assert_eq!(check_bearer(&bearer.state, &ok), Ok(()));
        let inventory = request_with_query(&format!("/api/v1/inventory?ticket={ticket}"));
        assert_eq!(
            check_bearer(&bearer.state, &inventory),
            Err("missing Authorization header")
        );
    }

    #[test]
    fn redact_removes_ticket_from_uri() {
        let mut request = request_with_query("/api/v1/camera/dock/stream?ticket=abc&x=1");
        let ticket = take_ticket_query(&mut request).unwrap();
        assert_eq!(ticket, "abc");
        assert_eq!(
            request.uri().path_and_query().unwrap(),
            "/api/v1/camera/dock/stream?x=1"
        );
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
    fn wrong_stream_ticket_is_rejected() {
        let bearer = state_with_bearer_token("secret");
        let request = request_with_query("/api/v1/camera/front-dock/snapshot?ticket=wrong");
        assert_eq!(
            check_bearer(&bearer.state, &request),
            Err("invalid or expired stream ticket")
        );
    }

    #[test]
    fn non_camera_paths_do_not_accept_query_tokens() {
        let bearer = state_with_bearer_token("secret");
        let request = request_with_query("/api/v1/inventory?token=secret");
        assert_eq!(
            check_bearer(&bearer.state, &request),
            Err("missing Authorization header")
        );
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
