// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use tokio::time::{interval, sleep, MissedTickBehavior};
use tracing::warn;

use crate::state::AppState;

pub async fn publisher_loop(state: Arc<AppState>) -> anyhow::Result<()> {
    let cfg = &state.config.nodra;
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

    let (client, mut eventloop) = AsyncClient::new(options, 64);
    let mut ticker = interval(Duration::from_secs(
        state.config.device.telemetry_interval_seconds.max(1),
    ));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut published_samples: BTreeMap<String, u64> = BTreeMap::new();
    let mut event_rx = state.subscribe_events();

    loop {
        tokio::select! {
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

                if state.config.industrial.publish_to_nodra {
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
