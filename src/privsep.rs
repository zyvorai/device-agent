// SPDX-License-Identifier: Apache-2.0

//! Privilege-separation helper scaffold for the daemon's *own* bus access.
//!
//! Plugin subprocess drops (`plugins.run_as_uid` / `run_as_gid`) already exist;
//! this module is about a future helper that would own GPIO/I2C/SPI/CAN device
//! opens while the API daemon runs unprivileged. See [`docs/PRIVSEP.md`](../docs/PRIVSEP.md)
//! and `docs/HARDWARE_PERMISSIONS.md`.
//!
//! **Default unchanged:** with `[privsep].enabled = false` (the default), this
//! module is a no-op and the stock root systemd unit keeps working. Even when
//! `enabled = true` and `--features privsep` is compiled in, the helper is
//! **not** spawned yet — only status / config validation surfaces exist.

use serde::Serialize;

use crate::config::PrivsepConfig;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PrivsepStatus {
    /// Config asked for a helper.
    pub enabled: bool,
    /// This binary was built with `--features privsep`.
    pub feature_compiled: bool,
    /// Helper process is actually running (always false in this scaffold).
    pub helper_running: bool,
    pub helper_path: String,
    pub socket_path: String,
    pub note: &'static str,
}

/// Compile-time: was `--features privsep` on?
pub const FEATURE_COMPILED: bool = cfg!(feature = "privsep");

/// Snapshot of whether a privsep helper would be used. Never claims the helper
/// is running — that requires a future spawn path.
pub fn status(config: &PrivsepConfig) -> PrivsepStatus {
    PrivsepStatus {
        enabled: config.enabled,
        feature_compiled: FEATURE_COMPILED,
        helper_running: false,
        helper_path: config.helper_path.clone(),
        socket_path: config.socket_path.clone(),
        note: if config.enabled && FEATURE_COMPILED {
            "privsep enabled in config and compiled in, but helper spawn is not implemented yet — daemon bus access unchanged"
        } else if config.enabled {
            "privsep.enabled set but this binary was built without --features privsep — rebuild to activate the scaffold; daemon bus access unchanged"
        } else {
            "privsep disabled (default) — daemon uses direct bus access as today"
        },
    }
}

/// Log once at startup when an operator turned the knob before the helper exists.
pub fn warn_if_requested(config: &PrivsepConfig) {
    if !config.enabled {
        return;
    }
    let snap = status(config);
    tracing::warn!(
        helper_path = %snap.helper_path,
        socket_path = %snap.socket_path,
        feature_compiled = snap.feature_compiled,
        "{}",
        snap.note
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PrivsepConfig;

    #[test]
    fn default_status_is_inert() {
        let snap = status(&PrivsepConfig::default());
        assert!(!snap.enabled);
        assert!(!snap.helper_running);
        assert_eq!(snap.feature_compiled, FEATURE_COMPILED);
        assert!(snap.note.contains("disabled"));
    }

    #[test]
    fn enabled_without_spawn_still_not_running() {
        let config = PrivsepConfig {
            enabled: true,
            helper_path: "/usr/lib/zyvor-device-agent/bus-helper".into(),
            socket_path: "/run/zyvor-device-agent/bus.sock".into(),
        };
        let snap = status(&config);
        assert!(snap.enabled);
        assert!(!snap.helper_running);
        assert!(!snap.helper_path.is_empty());
        assert!(!snap.socket_path.is_empty());
    }
}
