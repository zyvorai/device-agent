// SPDX-License-Identifier: Apache-2.0

//! Unix-domain-socket API listener with kernel-enforced peer-credential RBAC.
//! Additive to the TCP listener — same `Router`, same handlers, just a second
//! transport for same-host callers who'd rather not carry a bearer token.

use std::sync::Arc;

use axum::{
    extract::{connect_info::Connected, ConnectInfo, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    serve::IncomingStream,
    Json, Router,
};
use tokio::net::UnixListener;

use crate::state::AppState;

/// Captured once per accepted connection from `UnixStream::peer_cred()`.
#[derive(Debug, Clone, Copy)]
pub struct PeerCred {
    pub uid: u32,
    pub gid: u32,
}

impl Connected<IncomingStream<'_, UnixListener>> for PeerCred {
    fn connect_info(stream: IncomingStream<'_, UnixListener>) -> Self {
        match stream.io().peer_cred() {
            Ok(cred) => Self {
                uid: cred.uid(),
                gid: cred.gid(),
            },
            // peer_cred() failing on an already-accepted UDS connection is not
            // expected on Linux; fail closed rather than granting access.
            Err(error) => {
                tracing::warn!(%error, "failed to read UDS peer credentials");
                Self {
                    uid: u32::MAX,
                    gid: u32::MAX,
                }
            }
        }
    }
}

/// Wraps `base_router` (already built by `api::router`) with a peer-credential
/// check. Only ever attached to the router instance served over the Unix socket —
/// TCP connections never carry `ConnectInfo<PeerCred>`, so this middleware has
/// nothing to check for them (see `bind` below, which is the only caller).
fn with_peer_cred_check(base_router: Router, state: Arc<AppState>) -> Router {
    base_router.layer(middleware::from_fn_with_state(state, require_peer_cred))
}

async fn require_peer_cred(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<PeerCred>,
    request: Request,
    next: Next,
) -> Response {
    let cfg = &state.config.server.unix_socket;
    // Empty allow-lists mean nobody is allowed yet — the operator must explicitly
    // opt a uid/gid in. Prevents "I enabled the socket" from silently meaning
    // "and now anyone who can reach the path can call the API".
    let allowed = !(cfg.allow_uids.is_empty() && cfg.allow_gids.is_empty())
        && (cfg.allow_uids.contains(&peer.uid) || cfg.allow_gids.contains(&peer.gid));
    if allowed {
        next.run(request).await
    } else {
        tracing::warn!(
            uid = peer.uid,
            gid = peer.gid,
            "UDS peer rejected by allow-list"
        );
        (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "peer uid/gid not permitted on this socket" })),
        )
            .into_response()
    }
}

/// Binds the configured Unix socket path (removing a stale socket file first),
/// applies `file_mode`, and returns a `Router` ready to be passed to
/// `axum::serve(listener, router.into_make_service_with_connect_info::<PeerCred>())`.
pub fn bind(
    cfg: &crate::config::UnixSocketConfig,
    base_router: Router,
    state: Arc<AppState>,
) -> anyhow::Result<(UnixListener, Router)> {
    if let Some(parent) = std::path::Path::new(&cfg.path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    // A leftover socket file from a prior crash/unclean stop would otherwise fail bind()
    // with EADDRINUSE; safe to remove since UDS sockets aren't reused across restarts.
    let _ = std::fs::remove_file(&cfg.path);

    let listener = UnixListener::bind(&cfg.path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&cfg.path, std::fs::Permissions::from_mode(cfg.file_mode))?;
    }
    Ok((listener, with_peer_cred_check(base_router, state)))
}
