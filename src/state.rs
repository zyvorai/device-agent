// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::sync::{broadcast, RwLock};

use crate::{
    config::Config,
    model::{AgentEvent, AgentStatus, CanCaptureStatus, CanFrame, Inventory, SensorSample},
};

const EVENT_HISTORY_LIMIT: usize = 200;

pub struct AppState {
    pub config: Config,
    inventory: RwLock<Inventory>,
    samples: RwLock<BTreeMap<String, SensorSample>>,
    events: broadcast::Sender<AgentEvent>,
    event_history: Mutex<VecDeque<AgentEvent>>,
    can_frames: Mutex<VecDeque<CanFrame>>,
    can_frame_events: broadcast::Sender<CanFrame>,
    can_capture_last_error: Mutex<Option<String>>,
    nodra_connected: AtomicBool,
    inventory_generation: AtomicU64,
    last_inventory_refresh_unix_ms: AtomicU64,
    sensor_samples_total: AtomicU64,
    sensor_sample_failures: AtomicU64,
    event_sequence: AtomicU64,
    can_frame_sequence: AtomicU64,
    can_capture_frames_total: AtomicU64,
    can_capture_dropped_total: AtomicU64,
    can_capture_decode_errors_total: AtomicU64,
    bearer_token_hash: Option<[u8; 32]>,
    /// Keys of currently-breached thresholds (e.g. `"thermal:thermal_zone0:critical"`),
    /// used to emit a `threshold.breached`/`threshold.recovered` event only on the
    /// edge transition rather than every refresh tick a value stays over/under.
    breached_thresholds: Mutex<HashSet<String>>,
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
        let (can_frame_events, _) = broadcast::channel(1024);
        let can_history_limit = config.industrial.can_capture.history_limit.max(1);
        let bearer_token_hash = if config.auth.mode == "bearer" {
            crate::auth::bearer::load_token_hash(&config.auth.bearer.token_hash_file)
        } else {
            None
        };
        Self {
            config,
            inventory: RwLock::new(inventory),
            samples: RwLock::new(BTreeMap::new()),
            events,
            event_history: Mutex::new(VecDeque::with_capacity(EVENT_HISTORY_LIMIT)),
            can_frames: Mutex::new(VecDeque::with_capacity(can_history_limit)),
            can_frame_events,
            can_capture_last_error: Mutex::new(None),
            nodra_connected: AtomicBool::new(false),
            inventory_generation: AtomicU64::new(1),
            last_inventory_refresh_unix_ms: AtomicU64::new(now_unix_ms()),
            sensor_samples_total: AtomicU64::new(0),
            sensor_sample_failures: AtomicU64::new(0),
            event_sequence: AtomicU64::new(0),
            can_frame_sequence: AtomicU64::new(0),
            can_capture_frames_total: AtomicU64::new(0),
            can_capture_dropped_total: AtomicU64::new(0),
            can_capture_decode_errors_total: AtomicU64::new(0),
            bearer_token_hash,
            breached_thresholds: Mutex::new(HashSet::new()),
        }
    }

    pub fn bearer_token_hash(&self) -> Option<&[u8; 32]> {
        self.bearer_token_hash.as_ref()
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
        // Evaluated on every real change, not gated on `changed` below: a
        // temperature/error-counter value alone (no topology change) still
        // needs threshold checking every tick, but wouldn't otherwise appear
        // in `changed_sections` (which tracks presence/identity, not values).
        self.evaluate_thresholds(&inventory);
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

    /// Compares thermal zones and CAN controller error counters against
    /// `config.thresholds` (no-op if `thresholds.enabled` is false) and emits
    /// `threshold.breached`/`threshold.recovered` events on edge transitions
    /// only - a value that stays over/under a level doesn't re-emit every tick.
    fn evaluate_thresholds(&self, inventory: &Inventory) {
        let cfg = &self.config.thresholds;
        if !cfg.enabled {
            return;
        }

        let mut current: HashSet<String> = HashSet::new();

        for zone in &inventory.thermal {
            let Some(celsius) = zone.celsius else {
                continue;
            };
            let level = if celsius >= cfg.thermal_critical_celsius {
                Some("critical")
            } else if celsius >= cfg.thermal_warn_celsius {
                Some("warn")
            } else {
                None
            };
            if let Some(level) = level {
                let key = format!("thermal:{}:{level}", zone.name);
                self.note_threshold(&mut current, key, "thermal", &zone.name, celsius, level);
            }
        }

        for can in &inventory.industrial.can {
            let counter = can
                .rx_error_counter
                .unwrap_or(0)
                .max(can.tx_error_counter.unwrap_or(0));
            let level = if counter >= cfg.can_error_counter_critical {
                Some("critical")
            } else if counter >= cfg.can_error_counter_warn {
                Some("warn")
            } else {
                None
            };
            if let Some(level) = level {
                let key = format!("can:{}:{level}", can.name);
                self.note_threshold(&mut current, key, "can", &can.name, counter as f64, level);
            }
        }

        let Ok(mut breached) = self.breached_thresholds.lock() else {
            return;
        };
        for recovered in breached.difference(&current) {
            let mut parts = recovered.splitn(3, ':');
            let (Some(domain), Some(target)) = (parts.next(), parts.next()) else {
                continue;
            };
            self.emit_event(
                "threshold.recovered",
                serde_json::json!({ "domain": domain, "target": target }),
            );
        }
        *breached = current;
    }

    #[allow(clippy::too_many_arguments)]
    fn note_threshold(
        &self,
        current: &mut HashSet<String>,
        key: String,
        domain: &str,
        target: &str,
        value: f64,
        level: &str,
    ) {
        let is_new = !self
            .breached_thresholds
            .lock()
            .map(|breached| breached.contains(&key))
            .unwrap_or(false);
        if is_new {
            self.emit_event(
                "threshold.breached",
                serde_json::json!({
                    "domain": domain,
                    "target": target,
                    "level": level,
                    "value": value,
                }),
            );
        }
        current.insert(key);
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

    pub fn record_can_frame(&self, mut frame: CanFrame) {
        frame.sequence = self.can_frame_sequence.fetch_add(1, Ordering::Relaxed) + 1;
        self.can_capture_frames_total
            .fetch_add(1, Ordering::Relaxed);
        if let Ok(mut history) = self.can_frames.lock() {
            let limit = self.config.industrial.can_capture.history_limit.max(1);
            while history.len() >= limit {
                history.pop_front();
            }
            history.push_back(frame.clone());
        }
        let _ = self.can_frame_events.send(frame);
    }

    pub fn recent_can_frames(&self) -> Vec<CanFrame> {
        self.can_frames
            .lock()
            .map(|frames| frames.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn subscribe_can_frames(&self) -> broadcast::Receiver<CanFrame> {
        self.can_frame_events.subscribe()
    }

    pub fn note_can_capture_dropped(&self) {
        self.can_capture_dropped_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn note_can_capture_decode_error(&self) {
        self.can_capture_decode_errors_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn note_can_capture_started(&self, interface: &str) {
        self.emit_event(
            "can.capture.started",
            serde_json::json!({"interface": interface, "read_only": true}),
        );
    }

    pub fn note_can_capture_error(&self, interface: &str, error: &str) {
        if let Ok(mut last) = self.can_capture_last_error.lock() {
            *last = Some(format!("{interface}: {error}"));
        }
        self.emit_event(
            "can.capture.error",
            serde_json::json!({"interface": interface, "error": error}),
        );
    }

    pub fn can_capture_status(&self) -> CanCaptureStatus {
        CanCaptureStatus {
            enabled: self.config.industrial.can_capture.enabled,
            interfaces: self.config.industrial.can_capture.interfaces.clone(),
            frames_total: self.can_capture_frames_total.load(Ordering::Relaxed),
            dropped_total: self.can_capture_dropped_total.load(Ordering::Relaxed),
            decode_errors_total: self.can_capture_decode_errors_total.load(Ordering::Relaxed),
            history_len: self
                .can_frames
                .lock()
                .map(|frames| frames.len())
                .unwrap_or_default(),
            subscribers: self.can_frame_events.receiver_count(),
            last_error: self
                .can_capture_last_error
                .lock()
                .ok()
                .and_then(|value| value.clone()),
        }
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
            can_capture_frames_total: self.can_capture_frames_total.load(Ordering::Relaxed),
            can_capture_dropped_total: self.can_capture_dropped_total.load(Ordering::Relaxed),
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
    if industrial_topology_changed(old, new) {
        changed.push("industrial");
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

fn industrial_topology_changed(old: &Inventory, new: &Inventory) -> bool {
    let old_can = old
        .industrial
        .can
        .iter()
        .map(|interface| {
            (
                &interface.name,
                &interface.kind,
                &interface.operstate,
                &interface.driver,
                &interface.bitrate,
                &interface.data_bitrate,
                &interface.can_state,
                &interface.restart_ms,
                &interface.controller_modes,
            )
        })
        .collect::<Vec<_>>();
    let new_can = new
        .industrial
        .can
        .iter()
        .map(|interface| {
            (
                &interface.name,
                &interface.kind,
                &interface.operstate,
                &interface.driver,
                &interface.bitrate,
                &interface.data_bitrate,
                &interface.can_state,
                &interface.restart_ms,
                &interface.controller_modes,
            )
        })
        .collect::<Vec<_>>();
    if old_can != new_can {
        return true;
    }

    let old_serial = old
        .industrial
        .serial
        .iter()
        .map(|port| {
            (
                &port.name,
                &port.path,
                &port.driver,
                &port.transport,
                &port.rs485,
            )
        })
        .collect::<Vec<_>>();
    let new_serial = new
        .industrial
        .serial
        .iter()
        .map(|port| {
            (
                &port.name,
                &port.path,
                &port.driver,
                &port.transport,
                &port.rs485,
            )
        })
        .collect::<Vec<_>>();
    old_serial != new_serial
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DeviceIdentity, SystemInfo, ThermalZone};

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

    fn test_config(enabled: bool) -> Config {
        let mut cfg = Config::default();
        cfg.thresholds.enabled = enabled;
        cfg.thresholds.thermal_warn_celsius = 75.0;
        cfg.thresholds.thermal_critical_celsius = 90.0;
        cfg
    }

    #[tokio::test]
    async fn thresholds_disabled_by_default_emit_nothing() {
        let state = AppState::new(Config::default(), empty_inventory());
        let mut hot = empty_inventory();
        hot.thermal.push(ThermalZone {
            name: "zone0".into(),
            kind: "cpu".into(),
            celsius: Some(120.0),
        });
        state.update_inventory(hot).await;
        assert!(state
            .recent_events()
            .iter()
            .all(|event| !event.kind.starts_with("threshold.")));
    }

    #[tokio::test]
    async fn thermal_breach_emits_once_then_recovers() {
        let state = AppState::new(test_config(true), empty_inventory());

        let mut warm = empty_inventory();
        warm.thermal.push(ThermalZone {
            name: "zone0".into(),
            kind: "cpu".into(),
            celsius: Some(80.0),
        });
        state.update_inventory(warm.clone()).await;
        let breaches: Vec<_> = state
            .recent_events()
            .into_iter()
            .filter(|event| event.kind == "threshold.breached")
            .collect();
        assert_eq!(breaches.len(), 1);
        assert_eq!(breaches[0].data["level"], "warn");
        assert_eq!(breaches[0].data["domain"], "thermal");

        // Same value again: no repeat event on the edge that's already breached.
        let mut warm2 = warm.clone();
        warm2.system.uptime_seconds = 1; // force a real inventory diff so update_inventory proceeds
        state.update_inventory(warm2).await;
        let breach_count = state
            .recent_events()
            .into_iter()
            .filter(|event| event.kind == "threshold.breached")
            .count();
        assert_eq!(breach_count, 1);

        // Cool back down: expect a recovery event.
        let mut cool = empty_inventory();
        cool.thermal.push(ThermalZone {
            name: "zone0".into(),
            kind: "cpu".into(),
            celsius: Some(40.0),
        });
        state.update_inventory(cool).await;
        let recovered = state
            .recent_events()
            .into_iter()
            .any(|event| event.kind == "threshold.recovered" && event.data["target"] == "zone0");
        assert!(recovered);
    }
}
