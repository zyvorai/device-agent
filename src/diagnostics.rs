// SPDX-License-Identifier: Apache-2.0

//! Rules-based diagnostics. Findings explain evidence. They never remediate.

use serde::{Deserialize, Serialize};

use crate::model::{AgentEvent, Inventory};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub observation: String,
    pub evidence: Vec<String>,
    pub likely_causes: Vec<String>,
    pub recommended_tests: Vec<String>,
    pub confidence: String,
}

pub fn findings(
    inventory: &Inventory,
    events: &[AgentEvent],
    cert_not_after_unix: Option<i64>,
    now_unix: i64,
    nodra_connected: Option<bool>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    if let Some(finding) = can_bus_off(inventory, events) {
        out.push(finding);
    }
    if let Some(finding) = thermal(inventory) {
        out.push(finding);
    }
    if nodra_connected == Some(false)
        || events
            .iter()
            .any(|event| event.kind == "nodra.disconnected")
    {
        out.push(Finding {
            observation: "Nodra connectivity was lost".into(),
            evidence: vec!["nodra.disconnected was observed or the publisher is down".into()],
            likely_causes: vec![
                "WAN loss".into(),
                "broker unreachable".into(),
                "credential or TLS failure".into(),
            ],
            recommended_tests: vec![
                "confirm the broker address".into(),
                "review the flight recorder around the disconnect".into(),
            ],
            confidence: "medium".into(),
        });
    }
    if let Some(not_after) = cert_not_after_unix {
        if not_after <= now_unix + 14 * 86400 {
            out.push(Finding {
                observation: "Device certificate is expired or inside the renewal window".into(),
                evidence: vec![format!("not_after_unix={not_after} now_unix={now_unix}")],
                likely_causes: vec!["certificate lifetime elapsed".into()],
                recommended_tests: vec!["run agentctl enroll --renew".into()],
                confidence: "high".into(),
            });
        }
    }
    out
}

fn can_bus_off(inventory: &Inventory, events: &[AgentEvent]) -> Option<Finding> {
    let bus_off = inventory.industrial.can.iter().find(|iface| {
        iface
            .can_state
            .as_deref()
            .unwrap_or("")
            .eq_ignore_ascii_case("BUS-OFF")
    });
    let progressed = events.iter().any(|event| {
        event.kind.contains("can")
            && event.data.to_string().contains("BUS-OFF")
            && event.data.to_string().contains("ERROR-PASSIVE")
    });
    let iface = bus_off?;
    let rx = iface.rx_errors.unwrap_or(0);
    Some(Finding {
        observation: format!("{} transitioned toward BUS-OFF", iface.name),
        evidence: vec![
            format!("controller state {:?}", iface.can_state),
            format!("rx_errors={rx}"),
            format!("event progression recorded={progressed}"),
        ],
        likely_causes: vec![
            "termination".into(),
            "cabling".into(),
            "bitrate mismatch".into(),
            "transceiver power".into(),
        ],
        recommended_tests: vec![
            "inspect termination".into(),
            "validate configured bitrate".into(),
            "run passive capture".into(),
        ],
        confidence: if rx > 0 || progressed {
            "high"
        } else {
            "medium"
        }
        .into(),
    })
}

fn thermal(inventory: &Inventory) -> Option<Finding> {
    let hot = inventory
        .thermal
        .iter()
        .filter_map(|zone| zone.celsius.map(|c| (zone.name.as_str(), c)))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?;
    if hot.1 < 90.0 {
        return None;
    }
    Some(Finding {
        observation: format!("{} reached {:.1}°C", hot.0, hot.1),
        evidence: vec![format!("thermal zone {} celsius={}", hot.0, hot.1)],
        likely_causes: vec![
            "blocked airflow".into(),
            "failed fan".into(),
            "high ambient temperature".into(),
        ],
        recommended_tests: vec![
            "compare with the flight recorder thermal events".into(),
            "inspect the heatsink and airflow".into(),
        ],
        confidence: "high".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CanInterfaceInfo, DeviceIdentity, Inventory, SystemInfo, ThermalZone};

    fn base() -> Inventory {
        Inventory {
            device: DeviceIdentity {
                serial: "t".into(),
                vendor: "t".into(),
                model: "t".into(),
                hostname: "t".into(),
                machine_id: "t".into(),
            },
            system: SystemInfo {
                arch: "x".into(),
                kernel: "x".into(),
                os: "x".into(),
                cpu_model: "x".into(),
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

    #[test]
    fn bus_off_fixture_is_high_confidence() {
        let raw = include_str!("../fixtures/diagnostics/can-bus-off.json");
        let fixture: serde_json::Value = serde_json::from_str(raw).unwrap();
        let mut inventory = base();
        inventory.industrial.can.push(CanInterfaceInfo {
            name: "can0".into(),
            kind: "physical".into(),
            operstate: "up".into(),
            driver: None,
            mtu: None,
            bitrate: Some(500_000),
            data_bitrate: None,
            can_state: Some(fixture["state"].as_str().unwrap().into()),
            restart_ms: None,
            tx_error_counter: None,
            rx_error_counter: None,
            rx_bytes: None,
            tx_bytes: None,
            rx_errors: Some(fixture["rx_errors"].as_u64().unwrap()),
            tx_errors: None,
            rx_dropped: None,
            tx_dropped: None,
            controller_modes: vec![],
            details_source: "fixture".into(),
        });
        let events = vec![AgentEvent {
            id: 1,
            kind: "can.controller".into(),
            at_unix_ms: 43_000,
            data: serde_json::json!({"from":"ERROR-PASSIVE","to":"BUS-OFF"}),
        }];
        let found = findings(&inventory, &events, None, 0, Some(true));
        assert_eq!(found[0].confidence, "high");
        assert!(found[0].likely_causes.iter().any(|c| c == "termination"));
    }

    #[test]
    fn overheat_and_certificate_expiry() {
        let mut inventory = base();
        inventory.thermal.push(ThermalZone {
            name: "cpu".into(),
            kind: "soc".into(),
            celsius: Some(96.0),
        });
        let found = findings(&inventory, &[], Some(1_000), 1_000, Some(false));
        assert!(found.iter().any(|f| f.observation.contains("96.0")));
        assert!(found.iter().any(|f| f.observation.contains("certificate")));
        assert!(found.iter().any(|f| f.observation.contains("Nodra")));
    }
}
