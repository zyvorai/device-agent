// SPDX-License-Identifier: Apache-2.0

//! Library crate backing the `zyvor-device-agent` binary (`src/main.rs`), split out so
//! `tests/` can build the real `Router` and drive it end-to-end via
//! `tower::ServiceExt::oneshot` instead of only unit-testing individual functions.

pub mod api;
pub mod auth;
pub mod can_capture;
pub mod config;
pub mod hardware;
pub mod identity;
pub mod integrations;
pub mod model;
pub mod plugins;
pub mod profile;
pub mod state;
pub mod tls;
