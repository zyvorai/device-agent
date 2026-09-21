// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use arc_swap::{ArcSwap, ArcSwapOption};
use tokio::sync::{broadcast, RwLock};

use crate::{
    config::Config,
    model::{
        AgentEvent, AgentStatus, CameraCaptureStatus, CameraFrame, CanCaptureStatus, CanFrame,
        Inventory, SensorSample,
    },
};

const EVENT_HISTORY_LIMIT: usize = 200;

pub struct AppState {
    /// Hot-reloadable: `SIGHUP` re-reads the config file and stores a new
    /// `Config` here (see `main.rs`'s signal handling). `server.listen`,
    /// `server.unix_socket`, and `server.dashboard_dir` are captured once at
    /// listener-bind time in `main.rs::serve` and don't re-apply from a later
    /// store here — changing those still needs a restart.
    pub config: ArcSwap<Config>,
    /// Recomputed alongside `config` on every reload (see `reload_config`),
    /// since it's derived from `config.auth.bearer.token_hash_file` and would
    /// otherwise go stale the moment that file or `auth.mode` changes.
    bearer_token_hash: ArcSwapOption<[u8; 32]>,
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
    /// Keys of currently-breached thresholds (e.g. `"thermal:thermal_zone0:critical"`),
    /// used to emit a `threshold.breached`/`threshold.recovered` event only on the
    /// edge transition rather than every refresh tick a value stays over/under.
    breached_thresholds: Mutex<HashSet<String>>,
    /// Only the single newest frame per camera is ever useful to a live
    /// viewer - unlike `can_frames`, this is not a bounded history, since
    /// the daemon never warehouses video frames.
    camera_latest: Mutex<HashMap<String, Arc<CameraFrame>>>,
    /// One broadcast channel per *configured* camera id, pre-created in
    /// `AppState::new` (not lazily on first frame) so `subscribe_camera_frames`
    /// never races the capture thread's first send. A viewer wants one
    /// specific camera's feed, not all of them interleaved - unlike CAN,
    /// where every interface shares a single channel.
    camera_events: Mutex<HashMap<String, broadcast::Sender<Arc<CameraFrame>>>>,
    camera_capturing: Mutex<HashMap<String, bool>>,
    camera_last_error: Mutex<HashMap<String, String>>,
    camera_frames_total: Mutex<HashMap<String, u64>>,
    camera_dropped_total: Mutex<HashMap<String, u64>>,
    camera_encode_errors_total: Mutex<HashMap<String, u64>>,
    camera_last_frame_at_unix_ms: Mutex<HashMap<String, u64>>,
    stream_tickets: crate::auth::tickets::TicketStore,
}

/// Small on purpose: much smaller than CAN's 1024-frame channel
/// (`AppState::new`) - JPEG frames are large and only the latest one
/// matters to a live viewer, so a deep buffer just adds latency rather
/// than usefully queueing history.
const CAMERA_CHANNEL_CAPACITY: usize = 4;

pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn load_bearer_token_hash(config: &Config) -> Option<[u8; 32]> {
    if config.auth.mode == "bearer" {
        crate::auth::bearer::load_token_hash(&config.auth.bearer.token_hash_file)
    } else {
        None
    }
}

/// Config fields captured once at listener-bind time (`main.rs::serve`) that a
/// later `reload_config` store can't retroactively apply — used only to warn
/// the operator that a `SIGHUP` didn't pick up one of these.
fn restart_required_fields_changed(old: &Config, new: &Config) -> bool {
    old.server.listen != new.server.listen
        || old.server.unix_socket != new.server.unix_socket
        || old.server.dashboard_dir != new.server.dashboard_dir
}

impl AppState {
    pub fn new(config: Config, inventory: Inventory) -> Self {
        let (events, _) = broadcast::channel(256);
        let (can_frame_events, _) = broadcast::channel(1024);
        let can_history_limit = config.industrial.can_capture.history_limit.max(1);
        let bearer_token_hash = load_bearer_token_hash(&config);
        let camera_events = config
            .camera
            .devices
            .iter()
            .map(|device| {
                let (sender, _) = broadcast::channel(CAMERA_CHANNEL_CAPACITY);
                (device.id.clone(), sender)
            })
            .collect();
        Self {
            config: ArcSwap::new(Arc::new(config)),
            bearer_token_hash: ArcSwapOption::from(bearer_token_hash.map(Arc::new)),
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
            breached_thresholds: Mutex::new(HashSet::new()),
            camera_latest: Mutex::new(HashMap::new()),
            camera_events: Mutex::new(camera_events),
            camera_capturing: Mutex::new(HashMap::new()),
            camera_last_error: Mutex::new(HashMap::new()),
            camera_frames_total: Mutex::new(HashMap::new()),
            camera_dropped_total: Mutex::new(HashMap::new()),
            camera_encode_errors_total: Mutex::new(HashMap::new()),
            camera_last_frame_at_unix_ms: Mutex::new(HashMap::new()),
            stream_tickets: crate::auth::tickets::TicketStore::default(),
        }
    }

    pub fn stream_tickets(&self) -> &crate::auth::tickets::TicketStore {
        &self.stream_tickets
    }

    pub fn bearer_token_hash(&self) -> Option<[u8; 32]> {
        self.bearer_token_hash.load().as_deref().copied()
    }

    /// Applies a freshly re-read `Config` (from `SIGHUP`): stores it, recomputes
    /// the bearer-token hash from it, and reports whether a listener-affecting
    /// field changed (which this call can't apply - the caller should log that
    /// a restart is needed). Emits a `config.reloaded` event either way.
    pub fn reload_config(&self, new_config: Config) -> bool {
        let old_config = self.config.load_full();
        let restart_needed = restart_required_fields_changed(&old_config, &new_config);
        self.bearer_token_hash
            .store(load_bearer_token_hash(&new_config).map(Arc::new));
        self.config.store(Arc::new(new_config));
        self.emit_event(
            "config.reloaded",
            serde_json::json!({ "restart_required_fields_changed": restart_needed }),
        );
        restart_needed
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
        let config = self.config.load();
        let cfg = &config.thresholds;
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
            let limit = self
                .config
                .load()
                .industrial
                .can_capture
                .history_limit
                .max(1);
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
        let config = self.config.load();
        CanCaptureStatus {
            enabled: config.industrial.can_capture.enabled,
            interfaces: config.industrial.can_capture.interfaces.clone(),
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

    /// Assigns a sequence number (reusing the running frame count - every
    /// accepted frame increments it exactly once, so a separate counter
    /// would only duplicate it), updates the single-slot latest-frame
    /// cache, and broadcasts to any live `/stream` subscribers. A no-op if
    /// `camera_id` isn't a configured camera.
    pub fn record_camera_frame(&self, camera_id: &str, mut frame: CameraFrame) {
        let sequence = {
            let mut totals = match self.camera_frames_total.lock() {
                Ok(guard) => guard,
                Err(_) => return,
            };
            let entry = totals.entry(camera_id.to_string()).or_insert(0);
            *entry += 1;
            *entry
        };
        frame.sequence = sequence;
        let frame = Arc::new(frame);

        if let Ok(mut latest) = self.camera_latest.lock() {
            latest.insert(camera_id.to_string(), frame.clone());
        }
        if let Ok(mut last_frame_at) = self.camera_last_frame_at_unix_ms.lock() {
            last_frame_at.insert(camera_id.to_string(), now_unix_ms());
        }
        if let Ok(events) = self.camera_events.lock() {
            if let Some(sender) = events.get(camera_id) {
                let _ = sender.send(frame);
            }
        }
    }

    pub fn latest_camera_frame(&self, camera_id: &str) -> Option<Arc<CameraFrame>> {
        self.camera_latest
            .lock()
            .ok()
            .and_then(|latest| latest.get(camera_id).cloned())
    }

    /// `None` only when `camera_id` isn't a configured camera at all - the
    /// channel itself is pre-created for every configured device at
    /// `AppState::new`, so this never races the capture thread starting up.
    pub fn subscribe_camera_frames(
        &self,
        camera_id: &str,
    ) -> Option<broadcast::Receiver<Arc<CameraFrame>>> {
        self.camera_events
            .lock()
            .ok()
            .and_then(|events| events.get(camera_id).map(broadcast::Sender::subscribe))
    }

    pub fn camera_stream_subscriber_count(&self, camera_id: &str) -> usize {
        self.camera_events
            .lock()
            .ok()
            .and_then(|events| events.get(camera_id).map(broadcast::Sender::receiver_count))
            .unwrap_or(0)
    }

    pub fn is_configured_camera(&self, camera_id: &str) -> bool {
        self.camera_events
            .lock()
            .map(|events| events.contains_key(camera_id))
            .unwrap_or(false)
    }

    pub fn note_camera_capture_dropped(&self, camera_id: &str) {
        if let Ok(mut dropped) = self.camera_dropped_total.lock() {
            *dropped.entry(camera_id.to_string()).or_insert(0) += 1;
        }
    }

    pub fn note_camera_capture_encode_error(&self, camera_id: &str) {
        if let Ok(mut errors) = self.camera_encode_errors_total.lock() {
            *errors.entry(camera_id.to_string()).or_insert(0) += 1;
        }
    }

    pub fn note_camera_capture_started(&self, camera_id: &str) {
        if let Ok(mut capturing) = self.camera_capturing.lock() {
            capturing.insert(camera_id.to_string(), true);
        }
        if let Ok(mut last_error) = self.camera_last_error.lock() {
            last_error.remove(camera_id);
        }
        self.emit_event(
            "camera.capture.started",
            serde_json::json!({"camera_id": camera_id}),
        );
    }

    pub fn note_camera_capture_error(&self, camera_id: &str, error: &str) {
        if let Ok(mut capturing) = self.camera_capturing.lock() {
            capturing.insert(camera_id.to_string(), false);
        }
        if let Ok(mut last_error) = self.camera_last_error.lock() {
            last_error.insert(camera_id.to_string(), error.to_string());
        }
        self.emit_event(
            "camera.capture.error",
            serde_json::json!({"camera_id": camera_id, "error": error}),
        );
    }

    pub fn camera_capture_status(&self, camera_id: &str) -> Option<CameraCaptureStatus> {
        if !self.is_configured_camera(camera_id) {
            return None;
        }
        let config = self.config.load();
        let device_config = config
            .camera
            .devices
            .iter()
            .find(|device| device.id == camera_id)?;
        Some(CameraCaptureStatus {
            id: camera_id.to_string(),
            enabled: device_config.enabled,
            capturing: self
                .camera_capturing
                .lock()
                .ok()
                .and_then(|capturing| capturing.get(camera_id).copied())
                .unwrap_or(false),
            frames_total: self
                .camera_frames_total
                .lock()
                .ok()
                .and_then(|totals| totals.get(camera_id).copied())
                .unwrap_or(0),
            dropped_total: self
                .camera_dropped_total
                .lock()
                .ok()
                .and_then(|totals| totals.get(camera_id).copied())
                .unwrap_or(0),
            encode_errors_total: self
                .camera_encode_errors_total
                .lock()
                .ok()
                .and_then(|totals| totals.get(camera_id).copied())
                .unwrap_or(0),
            subscribers: self.camera_stream_subscriber_count(camera_id),
            last_error: self
                .camera_last_error
                .lock()
                .ok()
                .and_then(|errors| errors.get(camera_id).cloned()),
            last_frame_at_unix_ms: self
                .camera_last_frame_at_unix_ms
                .lock()
                .ok()
                .and_then(|frames| frames.get(camera_id).copied()),
        })
    }

    pub fn camera_capture_statuses(&self) -> Vec<CameraCaptureStatus> {
        self.config
            .load()
            .camera
            .devices
            .iter()
            .filter_map(|device| self.camera_capture_status(&device.id))
            .collect()
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
        let _ = self.events.send(event.clone());
        crate::recorder::record_agent_event(&self.config.load(), &event);
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

    #[test]
    fn reload_config_applies_hot_reloadable_fields() {
        let state = AppState::new(Config::default(), empty_inventory());
        assert!(!state.config.load().thresholds.enabled);

        let mut updated = Config::default();
        updated.thresholds.enabled = true;
        updated.thresholds.thermal_warn_celsius = 42.0;
        let restart_needed = state.reload_config(updated);

        assert!(!restart_needed);
        assert!(state.config.load().thresholds.enabled);
        assert_eq!(state.config.load().thresholds.thermal_warn_celsius, 42.0);
    }

    #[test]
    fn reload_config_flags_restart_needed_fields() {
        let state = AppState::new(Config::default(), empty_inventory());

        let mut updated = Config::default();
        updated.server.listen = "0.0.0.0:1".into();
        let restart_needed = state.reload_config(updated);

        assert!(restart_needed);
        // The new value is still stored - the daemon just can't apply it to an
        // already-bound listener without a restart.
        assert_eq!(state.config.load().server.listen, "0.0.0.0:1");
    }

    #[test]
    fn reload_config_recomputes_bearer_token_hash() {
        let state = AppState::new(Config::default(), empty_inventory());
        assert!(state.bearer_token_hash().is_none());

        let dir = TempDir::new();
        let hash_path = dir.0.join("bearer.sha256");
        std::fs::write(&hash_path, "a".repeat(64)).unwrap();

        let mut updated = Config::default();
        updated.auth.mode = "bearer".into();
        updated.auth.bearer.token_hash_file = hash_path.to_string_lossy().into_owned();
        state.reload_config(updated);

        assert_eq!(state.bearer_token_hash(), Some([0xaa; 32]));
    }

    /// Minimal scratch-directory helper, mirroring the one in `plugins.rs`'s
    /// own test module (kept local rather than shared, to not couple the two
    /// modules' test setup together for one small helper).
    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new() -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir =
                std::env::temp_dir().join(format!("zyvor-state-test-{}-{id}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
