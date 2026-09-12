// SPDX-License-Identifier: Apache-2.0

//! Optional accelerant on top of the existing polling inventory refresh
//! (`spawn_inventory_refresh` in `main.rs`, default every 5s): a raw
//! `NETLINK_KOBJECT_UEVENT` socket wakes an immediate re-scan when the
//! kernel reports a bus of interest appearing/disappearing, so hotplug
//! shows up well under the poll interval instead of waiting for it.
//!
//! Deliberately **not** `udev`/`tokio-udev`: those pull in a `libudev.so`
//! *runtime* dependency this codebase doesn't otherwise need (the
//! Dockerfile/.deb/.rpm don't ship it) just for a wake-up signal. A raw
//! netlink socket via `netlink-sys` (pure Rust, no C library) is enough to
//! read the same kernel-generated uevents udev itself listens for; only the
//! trivial `KEY=value\0`-separated line format needs hand-parsing.
//!
//! `--features hotplug`, off by default. Opening the netlink socket must
//! never be fatal to the daemon - insufficient capability (no
//! `CAP_NET_ADMIN`) or an unsupported platform just means falling back to
//! polling-only, logged once as a warning.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::state::AppState;

// Parsing helpers are compiled whenever the real listener is (to use) or
// under `cfg(test)` (so the parsing logic has real unit-test coverage even
// on platforms/configurations that never build the real listener itself).
#[cfg(any(all(feature = "hotplug", target_os = "linux"), test))]
mod uevent {
    /// Bus subsystems the existing hardware inventory actually cares about;
    /// anything else (e.g. plain block/cpu uevents) would just trigger a
    /// wasted re-scan.
    const RELEVANT_SUBSYSTEMS: &[&str] = &["gpio", "i2c", "spidev", "net", "usb", "tty"];

    /// Parses the `ACTION@DEVPATH\0KEY=VALUE\0KEY=VALUE\0...` uevent format,
    /// returning the `SUBSYSTEM` value if present.
    pub(super) fn parse_subsystem(datagram: &[u8]) -> Option<&str> {
        datagram
            .split(|&b| b == 0)
            // The first segment is the "ACTION@DEVPATH" header line, not a
            // KEY=VALUE pair.
            .skip(1)
            .find_map(|field| {
                let field = std::str::from_utf8(field).ok()?;
                field.strip_prefix("SUBSYSTEM=")
            })
    }

    pub(super) fn is_relevant(datagram: &[u8]) -> bool {
        parse_subsystem(datagram).is_some_and(|subsystem| RELEVANT_SUBSYSTEMS.contains(&subsystem))
    }
}

/// Kernel uevent multicast group (`man 7 netlink`'s `NETLINK_KOBJECT_UEVENT`
/// has a single group, bit 0) - distinct from udev's own userspace group,
/// which isn't relevant here since we only want the kernel's own events.
#[cfg(all(feature = "hotplug", target_os = "linux"))]
const KOBJECT_UEVENT_GROUP: u32 = 1;

#[cfg(all(feature = "hotplug", target_os = "linux"))]
pub fn spawn(state: Arc<AppState>, shutdown: CancellationToken) {
    use netlink_sys::{
        protocols::NETLINK_KOBJECT_UEVENT, AsyncSocket, AsyncSocketExt, SocketAddr, TokioSocket,
    };
    use tracing::{info, warn};

    tokio::spawn(async move {
        let mut socket = match TokioSocket::new(NETLINK_KOBJECT_UEVENT) {
            Ok(socket) => socket,
            Err(error) => {
                warn!(%error, "hotplug: failed to open netlink socket; falling back to polling-only inventory refresh");
                return;
            }
        };
        if let Err(error) = socket
            .socket_mut()
            .bind(&SocketAddr::new(0, KOBJECT_UEVENT_GROUP))
        {
            warn!(%error, "hotplug: failed to bind the kobject-uevent multicast group (needs CAP_NET_ADMIN); falling back to polling-only inventory refresh");
            return;
        }
        info!("hotplug: listening for kernel uevents alongside the polling inventory refresh");

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("hotplug listener shutting down");
                    break;
                }
                result = socket.recv_from_full() => {
                    match result {
                        Ok((datagram, _addr)) => {
                            if uevent::is_relevant(&datagram) {
                                let inventory = crate::hardware::collect_inventory(&state.config.load()).await;
                                state.update_inventory(inventory).await;
                            }
                        }
                        Err(error) => warn!(%error, "hotplug: netlink recv failed"),
                    }
                }
            }
        }
    });
}

#[cfg(not(all(feature = "hotplug", target_os = "linux")))]
pub fn spawn(_state: Arc<AppState>, _shutdown: CancellationToken) {}

#[cfg(test)]
mod tests {
    use super::uevent::{is_relevant, parse_subsystem};

    fn build_uevent(action_devpath: &str, pairs: &[(&str, &str)]) -> Vec<u8> {
        let mut datagram = action_devpath.as_bytes().to_vec();
        for (key, value) in pairs {
            datagram.push(0);
            datagram.extend_from_slice(format!("{key}={value}").as_bytes());
        }
        datagram
    }

    #[test]
    fn parses_subsystem_from_a_real_shaped_uevent() {
        let datagram = build_uevent(
            "add@/devices/platform/soc/i2c-1",
            &[
                ("ACTION", "add"),
                ("SUBSYSTEM", "i2c"),
                ("DEVNAME", "i2c-1"),
            ],
        );
        assert_eq!(parse_subsystem(&datagram), Some("i2c"));
        assert!(is_relevant(&datagram));
    }

    #[test]
    fn ignores_uevents_for_subsystems_inventory_does_not_track() {
        let datagram = build_uevent("change@/devices/system/cpu/cpu0", &[("SUBSYSTEM", "cpu")]);
        assert!(!is_relevant(&datagram));
    }

    #[test]
    fn ignores_uevents_with_no_subsystem_field() {
        let datagram = build_uevent("add@/devices/virtual/misc/foo", &[("ACTION", "add")]);
        assert_eq!(parse_subsystem(&datagram), None);
        assert!(!is_relevant(&datagram));
    }
}
