// SPDX-License-Identifier: Apache-2.0

use std::sync::atomic::{AtomicBool, Ordering};

use crate::{config::Config, model::Inventory};

pub struct AppState {
    pub config: Config,
    pub inventory: Inventory,
    nodra_connected: AtomicBool,
}

impl AppState {
    pub fn new(config: Config, inventory: Inventory) -> Self {
        Self { config, inventory, nodra_connected: AtomicBool::new(false) }
    }
    pub fn set_nodra_connected(&self, value: bool) { self.nodra_connected.store(value, Ordering::Relaxed); }
    pub fn nodra_connected(&self) -> bool { self.nodra_connected.load(Ordering::Relaxed) }
}
