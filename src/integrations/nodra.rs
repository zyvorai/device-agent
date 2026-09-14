// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, fs, sync::Arc, time::Duration};

use anyhow::{bail, Context};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS, Transport};
use tokio::time::{interval, sleep, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::config::NodraTlsConfig;
use crate::state::AppState;

pub async fn publisher_loop(
    state: Arc<AppState>,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    // Captured once at connection setup: broker/port/credentials/client_id and the
    // telemetry ticker interval below are structural to this MQTT connection and its
    // loop for its lifetime. A config reload (SIGHUP) won't reconnect or re-time
    // this loop - restart to pick up changes to `nodra.*`/`device.telemetry_interval_seconds`.
    let cfg = state.config.load().nodra.clone();
    let initial = state.inventory_snapshot().await;
    let prefix = format!(
        "{}/{}",
        cfg.topic_prefix.trim_end_matches('/'),
        initial.device.serial
    );

    let mut options = MqttOptions::new(&cfg.client_id, &cfg.broker, cfg.port);
    options.set_keep_alive(Duration::from_secs(30));
    if let (Some(username), Some(password)) = (&cfg.username, &cfg.password) {
        options.set_credentials(username.clone(), password.clone());
    }
    if let Some(transport) = mqtt_transport(&cfg.tls)? {
        options.set_transport(transport);
    }

    let (client, mut eventloop) = AsyncClient::new(options, 64);
    let mut ticker = interval(Duration::from_secs(
        state.config.load().device.telemetry_interval_seconds.max(1),
    ));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut published_samples: BTreeMap<String, u64> = BTreeMap::new();
    let mut event_rx = state.subscribe_events();
    let mut can_rx = state.subscribe_can_frames();

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("Nodra publisher loop shutting down");
                break;
            }
            event = eventloop.poll() => {
                match event {
                    Ok(Event::Incoming(Packet::ConnAck(_))) => state.set_nodra_connected(true),
                    Ok(Event::Incoming(_)) | Ok(Event::Outgoing(_)) => {},
                    Err(error) => {
                        state.set_nodra_connected(false);
                        warn!(?error, "Nodra MQTT connection error");
                        sleep(Duration::from_secs(2)).await;
                    }
                }
            }
            event = event_rx.recv() => {
                if let Ok(event) = event {
                    let topic = format!("{prefix}/events/{}", topic_segment(&event.kind));
                    let payload = serde_json::to_vec(&event)?;
                    publish(&client, topic, false, payload, &state).await;
                }
            }
            frame = can_rx.recv(), if state.config.load().industrial.can_capture.publish_to_nodra => {
                if let Ok(frame) = frame {
                    let topic = format!(
                        "{prefix}/industrial/can/{}/frames",
                        topic_segment(&frame.interface)
                    );
                    publish(&client, topic, false, serde_json::to_vec(&frame)?, &state).await;
                }
            }
            _ = ticker.tick() => {
                let inventory = state.inventory_snapshot().await;
                let inventory_payload = serde_json::to_vec(&inventory)?;
                publish(
                    &client,
                    format!("{prefix}/inventory"),
                    cfg.retain_inventory,
                    inventory_payload.clone(),
                    &state,
                ).await;
                publish(
                    &client,
                    format!("{prefix}/telemetry"),
                    false,
                    inventory_payload,
                    &state,
                ).await;

                let health = serde_json::to_vec(&serde_json::json!({
                    "status": "online",
                    "agent_version": env!("CARGO_PKG_VERSION"),
                    "inventory_generation": state.status().inventory_generation,
                }))?;
                publish(&client, format!("{prefix}/status"), true, health, &state).await;

                if state.config.load().industrial.publish_to_nodra {
                    let industrial = serde_json::to_vec(&inventory.industrial)?;
                    publish(
                        &client,
                        format!("{prefix}/industrial/status"),
                        true,
                        industrial,
                        &state,
                    ).await;
                    for can in &inventory.industrial.can {
                        publish(
                            &client,
                            format!("{prefix}/industrial/can/{}/status", topic_segment(&can.name)),
                            true,
                            serde_json::to_vec(can)?,
                            &state,
                        ).await;
                    }
                    for port in &inventory.industrial.serial {
                        publish(
                            &client,
                            format!("{prefix}/industrial/serial/{}/status", topic_segment(&port.name)),
                            true,
                            serde_json::to_vec(port)?,
                            &state,
                        ).await;
                    }
                }

                // Health/presence only, per-camera, never frame bytes - this is a
                // deliberate boundary, not an oversight: raw video must never reach
                // Nodra. Do not add a per-frame arm here mirroring `can_rx` above.
                for device in &state.config.load().camera.devices {
                    if !device.publish_to_nodra {
                        continue;
                    }
                    if let Some(camera_status) = state.camera_capture_status(&device.id) {
                        publish(
                            &client,
                            format!("{prefix}/camera/{}/status", topic_segment(&device.id)),
                            true,
                            serde_json::to_vec(&camera_status)?,
                            &state,
                        ).await;
                    }
                }

                for sample in state.latest_samples().await {
                    if !sample.publish_to_nodra {
                        continue;
                    }
                    let last = published_samples.get(&sample.sensor_id).copied().unwrap_or_default();
                    if sample.collected_at_unix_ms <= last {
                        continue;
                    }
                    let payload = serde_json::to_vec(&sample)?;
                    publish(
                        &client,
                        format!("{prefix}/sensors/{}", topic_segment(&sample.sensor_id)),
                        false,
                        payload,
                        &state,
                    ).await;
                    published_samples.insert(sample.sensor_id, sample.collected_at_unix_ms);
                }
            }
        }
    }
    Ok(())
}

/// Build rumqttc transport for Nodra MQTTS when `[nodra.tls] enabled = true`.
pub fn mqtt_transport(tls: &NodraTlsConfig) -> anyhow::Result<Option<Transport>> {
    if !tls.enabled {
        return Ok(None);
    }
    if !tls.cert_file.is_empty() && tls.key_file.is_empty() {
        bail!("nodra.tls.cert_file set but nodra.tls.key_file is empty");
    }
    if tls.ca_file.is_empty() && tls.cert_file.is_empty() {
        return Ok(Some(Transport::tls_with_default_config()));
    }
    let ca = if tls.ca_file.is_empty() {
        Vec::new()
    } else {
        fs::read(&tls.ca_file)
            .with_context(|| format!("reading nodra.tls.ca_file {}", tls.ca_file))?
    };
    let client_auth = if tls.cert_file.is_empty() {
        None
    } else {
        let cert = fs::read(&tls.cert_file)
            .with_context(|| format!("reading nodra.tls.cert_file {}", tls.cert_file))?;
        let key = fs::read(&tls.key_file)
            .with_context(|| format!("reading nodra.tls.key_file {}", tls.key_file))?;
        Some((cert, key))
    };
    if ca.is_empty() {
        if client_auth.is_some() {
            bail!("nodra.tls client certificates require nodra.tls.ca_file to be set");
        }
        return Ok(Some(Transport::tls_with_default_config()));
    }
    Ok(Some(Transport::tls(ca, client_auth, None)))
}

async fn publish(
    client: &AsyncClient,
    topic: String,
    retain: bool,
    payload: Vec<u8>,
    state: &AppState,
) {
    if let Err(error) = client
        .publish(topic, QoS::AtLeastOnce, retain, payload)
        .await
    {
        state.set_nodra_connected(false);
        warn!(?error, "Nodra publish failed");
    }
}

fn topic_segment(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NodraConfig;

    #[test]
    fn mqtt_transport_disabled_is_plain() {
        let tls = NodraTlsConfig::default();
        assert!(mqtt_transport(&tls).unwrap().is_none());
    }

    #[test]
    fn mqtt_transport_enabled_uses_default_roots() {
        let tls = NodraTlsConfig {
            enabled: true,
            ..NodraTlsConfig::default()
        };
        assert!(mqtt_transport(&tls).unwrap().is_some());
    }

    #[test]
    fn nodra_tls_deserializes_from_toml() {
        let raw = r#"
enabled = true
broker = "mqtt.example"
port = 8883
client_id = "da"
topic_prefix = "zyvor/device"
retain_inventory = true
[tls]
enabled = true
ca_file = "/etc/zyvor/mqtt-ca.pem"
"#;
        let cfg: NodraConfig = toml::from_str(raw).unwrap();
        assert!(cfg.tls.enabled);
        assert_eq!(cfg.tls.ca_file, "/etc/zyvor/mqtt-ca.pem");
        assert_eq!(cfg.port, 8883);
    }
}
