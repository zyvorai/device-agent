// SPDX-License-Identifier: Apache-2.0

use rumqttc::{AsyncClient, Event, MqttOptions, QoS};
use std::{sync::Arc, time::Duration};
use tokio::time::{interval, sleep};
use tracing::warn;

use crate::{hardware, state::AppState};

pub async fn publisher_loop(state: Arc<AppState>) -> anyhow::Result<()> {
    let cfg = &state.config.nodra;
    let mut options = MqttOptions::new(&cfg.client_id, &cfg.broker, cfg.port);
    options.set_keep_alive(Duration::from_secs(30));
    if let (Some(username), Some(password)) = (&cfg.username, &cfg.password) {
        options.set_credentials(username.clone(), password.clone());
    }

    let (client, mut eventloop) = AsyncClient::new(options, 32);
    let prefix = format!(
        "{}/{}",
        cfg.topic_prefix.trim_end_matches('/'),
        state.inventory.device.serial
    );
    let mut ticker = interval(Duration::from_secs(
        state.config.device.telemetry_interval_seconds.max(1),
    ));

    loop {
        tokio::select! {
            event = eventloop.poll() => {
                match event {
                    Ok(Event::Incoming(_)) | Ok(Event::Outgoing(_)) => state.set_nodra_connected(true),
                    Err(error) => {
                        state.set_nodra_connected(false);
                        warn!(?error, "Nodra MQTT connection error");
                        sleep(Duration::from_secs(2)).await;
                    }
                }
            }
            _ = ticker.tick() => {
                let inventory = hardware::collect_inventory(&state.config).await;
                let payload = serde_json::to_vec(&inventory)?;
                if let Err(error) = client.publish(format!("{prefix}/telemetry"), QoS::AtLeastOnce, false, payload).await {
                    state.set_nodra_connected(false);
                    warn!(?error, "Nodra publish failed");
                }
            }
        }
    }
}
