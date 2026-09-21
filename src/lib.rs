// SPDX-License-Identifier: Apache-2.0

//! Library crate backing the `zyvor-device-agent` binary (`src/main.rs`), split out so
//! `tests/` can build the real `Router` and drive it end-to-end via
//! `tower::ServiceExt::oneshot` instead of only unit-testing individual functions.

pub mod api;
pub mod auth;
pub mod bundle;
pub mod camera_capture;
pub mod can_capture;
pub mod config;
pub mod diagnostics;
#[cfg(feature = "edge-ai")]
pub mod edge_ai;
pub mod hardware;
pub mod identity;
pub mod inference_provider;
pub mod integrations;
pub mod model;
pub mod passport;
pub mod plugins;
pub mod plugins_sdk;
pub mod privsep;
pub mod profile;
pub mod recorder;
pub mod remediation;
pub mod signing;
pub mod state;
pub mod status_banner;
pub mod tls;
