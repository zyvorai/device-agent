// SPDX-License-Identifier: Apache-2.0

//! API auth. `mode = "none"` (default, backward compatible) enforces nothing. `mode =
//! "bearer"` requires `Authorization: Bearer <token>` on every request except
//! `auth.exempt_paths`. The daemon never generates or stores the raw token — only its
//! SHA-256 hash, loaded once at startup from `auth.bearer.token_hash_file` (see
//! scripts/deploy-remote.sh for how that file gets populated).

pub mod bearer;
pub mod uds;

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

use crate::state::AppState;

pub async fn dispatch(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if state
        .config
        .auth
        .exempt_paths
        .iter()
        .any(|exempt| exempt == path)
    {
        return next.run(request).await;
    }

    match state.config.auth.mode.as_str() {
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

fn check_bearer(state: &AppState, request: &Request) -> Result<(), &'static str> {
    let Some(expected) = state.bearer_token_hash() else {
        return Err("bearer auth enabled but no token is configured");
    };
    let Some(header_value) = request.headers().get(header::AUTHORIZATION) else {
        return Err("missing Authorization header");
    };
    let Ok(value) = header_value.to_str() else {
        return Err("invalid Authorization header");
    };
    let Some(token) = value.strip_prefix("Bearer ") else {
        return Err("Authorization header must use the Bearer scheme");
    };
    if bearer::verify(token, expected) {
        Ok(())
    } else {
        Err("invalid bearer token")
    }
}

fn unauthorized(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}
