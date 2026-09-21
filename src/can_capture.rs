// SPDX-License-Identifier: Apache-2.0

//! Read-only SocketCAN capture for diagnostics and Nodra hand-off.
//!
//! Capture is disabled by default and only binds interfaces explicitly listed in
//! configuration. The socket is never used for transmit and Device Agent never
//! changes bitrate, controller mode, or bus state.

use std::{
    ffi::{c_char, c_int, c_void, CString},
    mem,
    sync::Arc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::{model::CanFrame, state::AppState};

const PF_CAN: c_int = 29;
const SOCK_RAW: c_int = 3;
const CAN_RAW: c_int = 1;
const SOL_CAN_RAW: c_int = 101;
const CAN_RAW_ERR_FILTER: c_int = 2;
const CAN_RAW_FD_FRAMES: c_int = 5;
const SOL_SOCKET: c_int = 1;
const SO_RCVTIMEO: c_int = 20;

const CAN_EFF_FLAG: u32 = 0x8000_0000;
const CAN_RTR_FLAG: u32 = 0x4000_0000;
const CAN_ERR_FLAG: u32 = 0x2000_0000;
const CAN_SFF_MASK: u32 = 0x0000_07ff;
const CAN_EFF_MASK: u32 = 0x1fff_ffff;
const CAN_ERR_MASK: u32 = 0x1fff_ffff;
const CANFD_BRS: u8 = 0x01;
const CANFD_ESI: u8 = 0x02;

#[repr(C)]
struct SockAddrCan {
    can_family: u16,
    can_ifindex: i32,
    addr: [u8; 8],
}

#[repr(C)]
struct TimeVal {
    tv_sec: isize,
    tv_usec: isize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ClassicCanFrame {
    can_id: u32,
    len: u8,
    pad: u8,
    res0: u8,
    len8_dlc: u8,
    data: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CanFdFrame {
    can_id: u32,
    len: u8,
    flags: u8,
    res0: u8,
    res1: u8,
    data: [u8; 64],
}

unsafe extern "C" {
    fn socket(domain: c_int, socket_type: c_int, protocol: c_int) -> c_int;
    fn bind(fd: c_int, address: *const c_void, length: u32) -> c_int;
    fn recv(fd: c_int, buffer: *mut c_void, length: usize, flags: c_int) -> isize;
    fn setsockopt(
        fd: c_int,
        level: c_int,
        option: c_int,
        value: *const c_void,
        length: u32,
    ) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn if_nametoindex(name: *const c_char) -> u32;
}

pub fn spawn(state: Arc<AppState>, shutdown: CancellationToken) {
    // Captured once at startup: which interfaces to open sockets for is a
    // structural decision for this capture thread's lifetime. A config
    // reload (SIGHUP) updates `state.config` for other subsystems, but
    // changing `industrial.can_capture.*` still needs a restart to take
    // effect here.
    let cfg = state.config.load().industrial.can_capture.clone();
    if !cfg.enabled {
        return;
    }
    if cfg.interfaces.is_empty() {
        warn!("CAN capture enabled but no interfaces configured");
        return;
    }

    for interface in cfg.interfaces.clone() {
        let state = state.clone();
        let cfg = cfg.clone();
        let shutdown = shutdown.clone();
        let thread_name = interface.clone();
        thread::Builder::new()
            .name(format!("can-capture-{interface}"))
            .spawn(move || capture_loop(state, &interface, &cfg, shutdown))
            .unwrap_or_else(|error| {
                warn!(interface = %thread_name, ?error, "failed to start CAN capture thread");
                panic!("failed to start CAN capture thread: {error}")
            });
    }
}

fn capture_loop(
    state: Arc<AppState>,
    interface: &str,
    cfg: &crate::config::CanCaptureConfig,
    shutdown: CancellationToken,
) {
    let fd = match open_socket(interface, cfg.include_error_frames) {
        Ok(fd) => fd,
        Err(error) => {
            warn!(%interface, %error, "CAN capture unavailable");
            state.note_can_capture_error(interface, &error);
            return;
        }
    };

    info!(%interface, "read-only CAN capture started");
    state.note_can_capture_started(interface);

    let mut window_second = unix_seconds();
    let mut accepted_in_window = 0_u32;
    let max_fps = cfg.max_frames_per_second.max(1);

    while !shutdown.is_cancelled() {
        let mut raw = [0_u8; mem::size_of::<CanFdFrame>()];
        let read = unsafe { recv(fd, raw.as_mut_ptr().cast(), raw.len(), 0) };
        if read <= 0 {
            thread::sleep(Duration::from_millis(2));
            continue;
        }

        let now_second = unix_seconds();
        if now_second != window_second {
            window_second = now_second;
            accepted_in_window = 0;
        }
        if accepted_in_window >= max_fps {
            state.note_can_capture_dropped();
            continue;
        }

        let Some(frame) = decode(interface, &raw, read as usize) else {
            state.note_can_capture_decode_error();
            continue;
        };
        if frame.error && !cfg.include_error_frames {
            continue;
        }
        accepted_in_window += 1;
        state.record_can_frame(frame);
    }
    info!(%interface, "CAN capture shutting down");
    unsafe { close(fd) };
}

/// One receive-only read for the bus helper. Never transmits.
pub fn rx_once(interface: &str) -> Result<Option<CanFrame>, String> {
    let fd = open_socket(interface, false)?;
    let mut raw = [0_u8; mem::size_of::<CanFdFrame>()];
    let read = unsafe { recv(fd, raw.as_mut_ptr().cast(), raw.len(), 0) };
    unsafe { close(fd) };
    if read <= 0 {
        return Ok(None);
    }
    Ok(decode(interface, &raw, read as usize))
}

fn open_socket(interface: &str, include_error_frames: bool) -> Result<c_int, String> {
    let name = CString::new(interface).map_err(|_| "invalid interface name".to_string())?;
    let index = unsafe { if_nametoindex(name.as_ptr()) };
    if index == 0 {
        return Err(format!("interface {interface} not found"));
    }

    let fd = unsafe { socket(PF_CAN, SOCK_RAW, CAN_RAW) };
    if fd < 0 {
        return Err("socket(PF_CAN) failed".into());
    }

    let timeout = TimeVal {
        tv_sec: 0,
        tv_usec: 250_000,
    };
    let timeout_result = unsafe {
        setsockopt(
            fd,
            SOL_SOCKET,
            SO_RCVTIMEO,
            (&timeout as *const TimeVal).cast(),
            mem::size_of::<TimeVal>() as u32,
        )
    };
    if timeout_result != 0 {
        unsafe { close(fd) };
        return Err("setting CAN socket receive timeout failed".into());
    }

    let enable_fd: c_int = 1;
    unsafe {
        let _ = setsockopt(
            fd,
            SOL_CAN_RAW,
            CAN_RAW_FD_FRAMES,
            (&enable_fd as *const c_int).cast(),
            mem::size_of::<c_int>() as u32,
        );
    }

    if include_error_frames {
        let mask: u32 = CAN_ERR_MASK;
        let result = unsafe {
            setsockopt(
                fd,
                SOL_CAN_RAW,
                CAN_RAW_ERR_FILTER,
                (&mask as *const u32).cast(),
                mem::size_of::<u32>() as u32,
            )
        };
        if result != 0 {
            unsafe { close(fd) };
            return Err("enabling CAN error-frame reception failed".into());
        }
    }

    let address = SockAddrCan {
        can_family: PF_CAN as u16,
        can_ifindex: index as i32,
        addr: [0; 8],
    };
    let result = unsafe {
        bind(
            fd,
            (&address as *const SockAddrCan).cast(),
            mem::size_of::<SockAddrCan>() as u32,
        )
    };
    if result != 0 {
        unsafe { close(fd) };
        return Err(format!(
            "binding read-only CAN socket to {interface} failed"
        ));
    }
    Ok(fd)
}

fn decode(interface: &str, raw: &[u8], length: usize) -> Option<CanFrame> {
    if length == mem::size_of::<ClassicCanFrame>() {
        let frame = unsafe { std::ptr::read_unaligned(raw.as_ptr().cast::<ClassicCanFrame>()) };
        let data_len = usize::from(frame.len.min(8));
        return Some(build_frame(
            interface,
            frame.can_id,
            false,
            false,
            false,
            frame.len,
            &frame.data[..data_len],
        ));
    }
    if length == mem::size_of::<CanFdFrame>() {
        let frame = unsafe { std::ptr::read_unaligned(raw.as_ptr().cast::<CanFdFrame>()) };
        let data_len = usize::from(frame.len.min(64));
        return Some(build_frame(
            interface,
            frame.can_id,
            true,
            frame.flags & CANFD_BRS != 0,
            frame.flags & CANFD_ESI != 0,
            frame.len,
            &frame.data[..data_len],
        ));
    }
    None
}

fn build_frame(
    interface: &str,
    raw_id: u32,
    fd: bool,
    bitrate_switch: bool,
    error_state_indicator: bool,
    dlc: u8,
    data: &[u8],
) -> CanFrame {
    let extended = raw_id & CAN_EFF_FLAG != 0;
    let identifier = if extended {
        raw_id & CAN_EFF_MASK
    } else {
        raw_id & CAN_SFF_MASK
    };
    let remote = raw_id & CAN_RTR_FLAG != 0;
    let payload = if remote { &[][..] } else { data };
    CanFrame {
        sequence: 0,
        interface: interface.to_string(),
        captured_at_unix_ms: crate::state::now_unix_ms(),
        can_id: identifier,
        extended,
        remote,
        error: raw_id & CAN_ERR_FLAG != 0,
        fd,
        bitrate_switch,
        error_state_indicator,
        dlc,
        data: payload.to_vec(),
        data_hex: payload
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(""),
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_classic_extended_frame() {
        let frame = ClassicCanFrame {
            can_id: CAN_EFF_FLAG | 0x18ff50e5,
            len: 3,
            pad: 0,
            res0: 0,
            len8_dlc: 0,
            data: [0x01, 0xa2, 0xff, 0, 0, 0, 0, 0],
        };
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (&frame as *const ClassicCanFrame).cast::<u8>(),
                mem::size_of::<ClassicCanFrame>(),
            )
        };
        let decoded = decode("can0", bytes, bytes.len()).unwrap();
        assert!(decoded.extended);
        assert_eq!(decoded.can_id, 0x18ff50e5);
        assert_eq!(decoded.data, vec![0x01, 0xa2, 0xff]);
        assert_eq!(decoded.data_hex, "01A2FF");
    }

    #[test]
    fn decodes_can_fd_flags() {
        let frame = CanFdFrame {
            can_id: 0x123,
            len: 12,
            flags: CANFD_BRS | CANFD_ESI,
            res0: 0,
            res1: 0,
            data: [0x5a; 64],
        };
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (&frame as *const CanFdFrame).cast::<u8>(),
                mem::size_of::<CanFdFrame>(),
            )
        };
        let decoded = decode("can1", bytes, bytes.len()).unwrap();
        assert!(decoded.fd);
        assert!(decoded.bitrate_switch);
        assert!(decoded.error_state_indicator);
        assert_eq!(decoded.data.len(), 12);
    }
}
