// SPDX-License-Identifier: Apache-2.0

//! Edge AI inference bridge scaffold (`--features edge-ai`).
//!
//! Completes the contract half of the camera → inference path described in
//! [`docs/EDGE_AI.md`](../docs/EDGE_AI.md) and `docs/ROADMAP.md`. This module
//! deliberately does **not** open an NPU/GPU/accelerator or run models: it
//! only exposes a stable placeholder HTTP surface so Fleet/Nodra clients can
//! wire against the path early.
//!
//! Without an accelerator backend configured (today: always), handlers return
//! HTTP 501 with `error = "not-configured"`.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::config::EdgeAiConfig;

/// Wire contract version for `/api/v1/inference/*`. Bump when the JSON shape
/// of successful event payloads changes (501 scaffold responses stay compatible).
pub const CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InferenceNotConfigured {
    pub error: &'static str,
    pub message: String,
    pub contract_version: u32,
    pub configured: bool,
    pub accelerator: Option<String>,
    pub publish_to_nodra: bool,
}

/// Build the 501 body used when no accelerator is configured / ready.
pub fn not_configured_body(config: &EdgeAiConfig) -> InferenceNotConfigured {
    let accelerator = if config.accelerator.is_empty() {
        None
    } else {
        Some(config.accelerator.clone())
    };
    InferenceNotConfigured {
        error: "not-configured",
        message: "Edge AI inference is not configured: no accelerator backend is enabled".into(),
        contract_version: CONTRACT_VERSION,
        configured: false,
        accelerator,
        publish_to_nodra: config.publish_to_nodra,
    }
}

/// `GET /api/v1/inference/events` — placeholder for a future SSE/event stream
/// of local inference results into Nodra. Always 501 until a real backend
/// lands.
pub async fn inference_events(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<crate::state::AppState>>,
) -> Response {
    let config = state.config.load().edge_ai.clone();
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(not_configured_body(&config)),
    )
        .into_response()
}

/// Whether this build + config would attempt real inference (never true today).
pub fn is_runtime_ready(config: &EdgeAiConfig) -> bool {
    // Scaffold: even with `enabled = true` and a named accelerator family,
    // there is no backend to open — keep the daemon's default path inert.
    let _ = config;
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EdgeAiConfig;

    #[test]
    fn not_configured_body_defaults() {
        let body = not_configured_body(&EdgeAiConfig::default());
        assert_eq!(body.error, "not-configured");
        assert_eq!(body.contract_version, CONTRACT_VERSION);
        assert!(!body.configured);
        assert!(body.accelerator.is_none());
        assert!(!is_runtime_ready(&EdgeAiConfig::default()));
    }

    #[test]
    fn not_configured_preserves_declared_accelerator_name() {
        let config = EdgeAiConfig {
            enabled: true,
            accelerator: "rknn".into(),
            publish_to_nodra: true,
        };
        let body = not_configured_body(&config);
        assert_eq!(body.accelerator.as_deref(), Some("rknn"));
        assert!(body.publish_to_nodra);
        // Declaring a family name does not make a backend appear.
        assert!(!is_runtime_ready(&config));
    }
}
