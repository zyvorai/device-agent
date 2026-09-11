// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::sync::{broadcast, RwLock};

use crate::{
    config::Config,
    model::{AgentEvent, AgentStatus, Inventory, SensorSample},
};

const EVENT_HISTORY_LIMIT: usize = 200;

pub struct AppState {
    pub config: Config,
    inventory: RwLock<Inventory>,
    samples: RwLock<BTreeMap<String, SensorSample>>,
    events: broadcast::Sender<AgentEvent>,
    event_history: Mutex<VecDeque<AgentEvent>>,
    nodra_connected: AtomicBool,
    inventory_generation: AtomicU64,
    last_inventory_refresh_unix_ms: AtomicU64,
    sensor_samples_total: AtomicU64,
    sensor_sample_failures: AtomicU64,
    event_sequence: AtomicU64,
}

pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

impl AppState {
    pub fn new(config: Config, inventory: Inventory) -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            config,
            inventory: RwLock::new(inventory),
            samples: RwLock::new(BTreeMap::new()),
            events,
            event_history: Mutex::new(VecDeque::with_capacity(EVENT_HISTORY_LIMIT)),
            nodra_connected: AtomicBool::new(false),
            inventory_generation: AtomicU64::new(1),
            last_inventory_refresh_unix_ms: AtomicU64::new(now_unix_ms()),
            sensor_samples_total: AtomicU64::new(0),
            sensor_sample_failures: AtomicU64::new(0),
            event_sequence: AtomicU64::new(0),
        }
    }

    pub async fn inventory_snapshot(&self) -> Inventory {
        self.inventory.read().await.clone()
    }

    pub async fn update_inventory(&self, inventory: Inventory) -> bool {
        let mut current = self.inventory.write().await;
        self.last_inventory_refresh_unix_ms
            .store(now_unix_ms(), Ordering::Relaxed);
        if *current == inventory {
            return false;
        }

        let changed = changed_sections(&current, &inventory);
        *current = inventory;
        drop(current);
        if changed.is_empty() {
            return false;
        }

        let generation = self.inventory_generation.fetch_add(1, Ordering::Relaxed) + 1;
        self.emit_event(
            "inventory.changed",
            serde_json::json!({
                "generation": generation,
                "sections": changed,
            }),
        );
        true
    }

    pub async fn record_sample(&self, sample: SensorSample) {
        self.sensor_samples_total.fetch_add(1, Ordering::Relaxed);
        if !sample.ok {
            self.sensor_sample_failures.fetch_add(1, Ordering::Relaxed);
        }
        self.samples
            .write()
            .await
            .insert(sample.sensor_id.clone(), sample.clone());
        self.emit_event(
            "sensor.sample",
            serde_json::to_value(sample).unwrap_or_default(),
        );
    }

    pub async fn latest_samples(&self) -> Vec<SensorSample> {
        self.samples.read().await.values().cloned().collect()
    }

    pub async fn latest_sample(&self, sensor_id: &str) -> Option<SensorSample> {
        self.samples.read().await.get(sensor_id).cloned()
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<AgentEvent> {
        self.events.subscribe()
    }

    pub fn recent_events(&self) -> Vec<AgentEvent> {
        self.event_history
            .lock()
            .map(|events| events.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn emit_event(&self, kind: &str, data: serde_json::Value) {
        let event = AgentEvent {
            id: self.event_sequence.fetch_add(1, Ordering::Relaxed) + 1,
            kind: kind.to_string(),
            at_unix_ms: now_unix_ms(),
            data,
        };

        if let Ok(mut history) = self.event_history.lock() {
            if history.len() == EVENT_HISTORY_LIMIT {
                history.pop_front();
            }
            history.push_back(event.clone());
        }
        let _ = self.events.send(event);
    }

    pub fn set_nodra_connected(&self, value: bool) {
        let previous = self.nodra_connected.swap(value, Ordering::Relaxed);
        if previous != value {
            self.emit_event(
                if value {
                    "nodra.connected"
                } else {
                    "nodra.disconnected"
                },
                serde_json::json!({ "connected": value }),
            );
        }
    }

    pub fn nodra_connected(&self) -> bool {
        self.nodra_connected.load(Ordering::Relaxed)
    }

    pub fn status(&self) -> AgentStatus {
        AgentStatus {
            version: env!("CARGO_PKG_VERSION").to_string(),
            inventory_generation: self.inventory_generation.load(Ordering::Relaxed),
            last_inventory_refresh_unix_ms: self
                .last_inventory_refresh_unix_ms
                .load(Ordering::Relaxed),
            sensor_samples_total: self.sensor_samples_total.load(Ordering::Relaxed),
            sensor_sample_failures: self.sensor_sample_failures.load(Ordering::Relaxed),
            event_subscribers: self.events.receiver_count(),
        }
    }
}

fn changed_sections(old: &Inventory, new: &Inventory) -> Vec<&'static str> {
    let mut changed = Vec::new();
    if old.device != new.device {
        changed.push("device");
    }
    if system_identity_changed(old, new) {
        changed.push("system");
    }
    if network_topology_changed(old, new) {
        changed.push("network");
    }
    if old.buses != new.buses {
        changed.push("buses");
    }
    if old.usb != new.usb {
        changed.push("usb");
    }
    if thermal_topology_changed(old, new) {
        changed.push("thermal");
    }
    if old.capabilities != new.capabilities {
        changed.push("capabilities");
    }
    changed
}

fn system_identity_changed(old: &Inventory, new: &Inventory) -> bool {
    old.system.arch != new.system.arch
        || old.system.kernel != new.system.kernel
        || old.system.os != new.system.os
        || old.system.cpu_model != new.system.cpu_model
        || old.system.cpu_cores != new.system.cpu_cores
        || old.system.memory_bytes != new.system.memory_bytes
        || old.system.storage_bytes != new.system.storage_bytes
}

fn network_topology_changed(old: &Inventory, new: &Inventory) -> bool {
    let old_view = old
        .network
        .iter()
        .map(|interface| {
            (
                &interface.name,
                &interface.kind,
                &interface.operstate,
                &interface.mac,
                &interface.mtu,
                &interface.addresses,
            )
        })
        .collect::<Vec<_>>();
    let new_view = new
        .network
        .iter()
        .map(|interface| {
            (
                &interface.name,
                &interface.kind,
                &interface.operstate,
                &interface.mac,
                &interface.mtu,
                &interface.addresses,
            )
        })
        .collect::<Vec<_>>();
    old_view != new_view
}

fn thermal_topology_changed(old: &Inventory, new: &Inventory) -> bool {
    let old_view = old
        .thermal
        .iter()
        .map(|zone| (&zone.name, &zone.kind))
        .collect::<Vec<_>>();
    let new_view = new
        .thermal
        .iter()
        .map(|zone| (&zone.name, &zone.kind))
        .collect::<Vec<_>>();
    old_view != new_view
}
