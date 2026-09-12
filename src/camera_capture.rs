// SPDX-License-Identifier: Apache-2.0

//! Opt-in, explicitly-allowlisted camera capture — one dedicated OS thread
//! per configured camera, mirroring `can_capture.rs`'s posture: never
//! auto-opens an undeclared `/dev/video*`, never fatal to the daemon,
//! rate-limited.
//!
//! `--features camera` (off by default, matching `hotplug`/`tpm2`): pulls in
//! `v4l` (pure-Rust V4L2 ioctls, default `v4l2` feature — no `libv4l.so`
//! link) and `jpeg-encoder` (pure-Rust baseline JPEG encoder, used only for
//! the YUYV software-encode fallback path — a camera advertising native
//! MJPG has its own JPEG bytes passed straight through instead). V4L2
//! itself is Linux-only, so the device-opening/capture-loop code is
//! `#[cfg(target_os = "linux")]`-gated with a no-op `spawn()` stub
//! everywhere else, mirroring `hardware::hotplug`'s pattern exactly. The
//! YUYV→RGB→JPEG encode step has no V4L2 dependency at all, so it's kept in
//! its own cross-platform module, testable under `--features camera` on any
//! OS (including this project's own macOS dev machines).

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::state::AppState;

// Always requires `--features camera` (this module uses the optional
// `jpeg-encoder` crate, unlike `hardware::hotplug`'s pure-string-parsing
// test helpers, which need no optional dependency at all) - but within
// that, compiled on any OS under `cfg(test)`, not just Linux, so the
// YUYV->RGB->JPEG pipeline has real cross-platform unit-test coverage even
// on a dev machine (e.g. macOS) that never builds the real V4L2 capture
// loop at all.
#[cfg(all(feature = "camera", any(target_os = "linux", test)))]
mod encode {
    //! YUYV422→RGB→JPEG, with no V4L2/Linux dependency — the fallback path
    //! for cameras that don't advertise native MJPG.

    /// YUYV422 (aka YUY2) packs 2 pixels per 4 bytes as `Y0 U Y1 V`.
    /// Standard BT.601 YCbCr→RGB conversion, applied per pixel pair — the
    /// same matrix most UVC webcams' own ISPs assume by default.
    pub fn yuyv_to_rgb(yuyv: &[u8]) -> Vec<u8> {
        let mut rgb = Vec::with_capacity((yuyv.len() / 4) * 6);
        for chunk in yuyv.chunks_exact(4) {
            let (y0, u, y1, v) = (
                chunk[0] as f32,
                chunk[1] as f32,
                chunk[2] as f32,
                chunk[3] as f32,
            );
            for y in [y0, y1] {
                let c = y - 16.0;
                let d = u - 128.0;
                let e = v - 128.0;
                let r = (298.0 * c + 409.0 * e + 128.0) / 256.0;
                let g = (298.0 * c - 100.0 * d - 208.0 * e + 128.0) / 256.0;
                let b = (298.0 * c + 516.0 * d + 128.0) / 256.0;
                rgb.push(r.clamp(0.0, 255.0) as u8);
                rgb.push(g.clamp(0.0, 255.0) as u8);
                rgb.push(b.clamp(0.0, 255.0) as u8);
            }
        }
        rgb
    }

    pub fn encode_yuyv_to_jpeg(
        yuyv: &[u8],
        width: u32,
        height: u32,
        quality: u8,
    ) -> Result<Vec<u8>, String> {
        let rgb = yuyv_to_rgb(yuyv);
        let mut out = Vec::new();
        let encoder = jpeg_encoder::Encoder::new(&mut out, quality);
        encoder
            .encode(
                &rgb,
                width as u16,
                height as u16,
                jpeg_encoder::ColorType::Rgb,
            )
            .map_err(|error| error.to_string())?;
        Ok(out)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn neutral_chroma_gives_equal_rgb_channels() {
            // U=V=128 (neutral chroma) means R=G=B regardless of Y - BT.601's
            // limited range (16-235) rescales Y=128 to ~130.875, not 128, so
            // assert internal consistency (equal channels) and the exact
            // computed value, rather than a naive "input equals output".
            let yuyv = [128u8, 128, 128, 128];
            let rgb = yuyv_to_rgb(&yuyv);
            assert_eq!(rgb.len(), 6);
            assert_eq!(rgb[0], rgb[1]);
            assert_eq!(rgb[1], rgb[2]);
            assert_eq!(rgb, [130, 130, 130, 130, 130, 130]);
        }

        #[test]
        fn black_and_white_extremes() {
            // Y=16 is BT.601 black, Y=235 is BT.601 white, neutral chroma.
            let yuyv = [16u8, 128, 235, 128];
            let rgb = yuyv_to_rgb(&yuyv);
            assert!(rgb[0] <= 4, "pixel 0 should be near-black, got {}", rgb[0]);
            assert!(
                rgb[3] >= 251,
                "pixel 1 should be near-white, got {}",
                rgb[3]
            );
        }

        #[test]
        fn encodes_a_valid_jpeg() {
            let width = 4u32;
            let height = 2u32;
            let yuyv = vec![128u8; (width * height * 2) as usize];
            let jpeg = encode_yuyv_to_jpeg(&yuyv, width, height, 75).unwrap();
            assert!(!jpeg.is_empty());
            assert_eq!(&jpeg[0..2], [0xFF, 0xD8], "JPEG SOI marker");
        }
    }
}

#[cfg(all(feature = "camera", target_os = "linux"))]
mod linux {
    use std::{io, thread, time::Duration};

    use tracing::{info, warn};
    use v4l::{
        buffer::Type,
        format::FourCC,
        io::{mmap::Stream as MmapStream, traits::CaptureStream},
        video::Capture,
        Device, Format,
    };

    use super::encode::encode_yuyv_to_jpeg;
    use crate::{
        config::CameraDeviceConfig,
        model::CameraFrame,
        state::{now_unix_ms, AppState},
    };
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    const BUFFER_COUNT: u32 = 4;
    const DEQUEUE_TIMEOUT: Duration = Duration::from_millis(250);
    /// Bounded retry, unlike CAN capture's "fail once, never retry" policy —
    /// USB webcam replug is common enough to be worth recovering from
    /// automatically rather than needing a full daemon restart.
    const RETRY_BACKOFF: Duration = Duration::from_secs(5);

    pub fn spawn(state: Arc<AppState>, shutdown: CancellationToken) {
        let devices = state.config.load().camera.devices.clone();
        for device in devices {
            if !device.enabled {
                continue;
            }
            let state = state.clone();
            let shutdown = shutdown.clone();
            let camera_id = device.id.clone();
            let thread_name = format!("camera-capture-{camera_id}");
            if let Err(error) = thread::Builder::new()
                .name(thread_name)
                .spawn(move || run(&state, &shutdown, &device))
            {
                warn!(%camera_id, %error, "failed to spawn camera capture thread");
            }
        }
    }

    fn run(state: &Arc<AppState>, shutdown: &CancellationToken, device: &CameraDeviceConfig) {
        while !shutdown.is_cancelled() {
            if let Err(error) = open_and_capture(state, shutdown, device) {
                warn!(camera_id = %device.id, %error, "camera capture error, will retry");
                state.note_camera_capture_error(&device.id, &error.to_string());
                let mut waited = Duration::ZERO;
                while waited < RETRY_BACKOFF {
                    if shutdown.is_cancelled() {
                        return;
                    }
                    thread::sleep(Duration::from_millis(100));
                    waited += Duration::from_millis(100);
                }
            }
        }
    }

    fn open_and_capture(
        state: &Arc<AppState>,
        shutdown: &CancellationToken,
        device_cfg: &CameraDeviceConfig,
    ) -> io::Result<()> {
        let dev = Device::with_path(&device_cfg.path)?;
        let negotiated = negotiate_format(&dev)?;
        let is_mjpg = negotiated.fourcc.str() == Ok("MJPG");

        info!(
            camera_id = %device_cfg.id,
            path = %device_cfg.path,
            fourcc = %negotiated.fourcc,
            width = negotiated.width,
            height = negotiated.height,
            "camera capture starting"
        );

        let mut stream = MmapStream::with_buffers(&dev, Type::VideoCapture, BUFFER_COUNT)?;
        stream.set_timeout(DEQUEUE_TIMEOUT);
        state.note_camera_capture_started(&device_cfg.id);

        let mut window_start_ms = now_unix_ms();
        let mut accepted_in_window: u32 = 0;
        let max_fps = device_cfg.max_frames_per_second.max(1);

        while !shutdown.is_cancelled() {
            let (bytes, meta) = match stream.next() {
                Ok(pair) => pair,
                Err(error) if error.kind() == io::ErrorKind::TimedOut => continue,
                Err(error) => return Err(error),
            };

            let now = now_unix_ms();
            if now.saturating_sub(window_start_ms) >= 1000 {
                window_start_ms = now;
                accepted_in_window = 0;
            }
            // Checked before the (comparatively expensive) JPEG encode step,
            // not after — the inverse order from CAN's rate check, where
            // decode is the cheap part and encode isn't.
            if accepted_in_window >= max_fps {
                state.note_camera_capture_dropped(&device_cfg.id);
                continue;
            }
            accepted_in_window += 1;

            let used = (meta.bytesused as usize).min(bytes.len());
            let raw = &bytes[..used];

            let jpeg = if is_mjpg {
                raw.to_vec()
            } else {
                match encode_yuyv_to_jpeg(
                    raw,
                    negotiated.width,
                    negotiated.height,
                    device_cfg.jpeg_quality,
                ) {
                    Ok(jpeg) => jpeg,
                    Err(error) => {
                        warn!(camera_id = %device_cfg.id, %error, "camera JPEG encode failed");
                        state.note_camera_capture_encode_error(&device_cfg.id);
                        continue;
                    }
                }
            };

            state.record_camera_frame(
                &device_cfg.id,
                CameraFrame {
                    camera_id: device_cfg.id.clone(),
                    sequence: 0, // assigned by record_camera_frame
                    captured_at_unix_ms: now,
                    width: negotiated.width,
                    height: negotiated.height,
                    content_type: "image/jpeg",
                    jpeg,
                },
            );
        }

        Ok(())
    }

    /// Prefers native MJPG (pass the driver's own JPEG bytes straight
    /// through — cheapest path); falls back to YUYV (near-universal on UVC
    /// webcams) for software encoding. A device offering neither is
    /// rejected outright — no daemon-side support for other raw/compressed
    /// formats in this increment, and no exposure/white-balance/other
    /// `VIDIOC_S_CTRL` control tuning either: this is acquisition only,
    /// matching Device Agent's bounded, opt-in-access charter.
    fn negotiate_format(dev: &Device) -> io::Result<Format> {
        let current = dev.format()?;
        let available = dev.enum_formats()?;

        let mjpg = FourCC::new(b"MJPG");
        let yuyv = FourCC::new(b"YUYV");

        let chosen = if available.iter().any(|d| d.fourcc.repr == mjpg.repr) {
            mjpg
        } else if available.iter().any(|d| d.fourcc.repr == yuyv.repr) {
            yuyv
        } else {
            let offered = available
                .iter()
                .map(|d| d.fourcc.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("camera advertises neither MJPG nor YUYV (available: {offered})"),
            ));
        };

        let requested = Format::new(current.width, current.height, chosen);
        dev.set_format(&requested)
    }
}

#[cfg(all(feature = "camera", target_os = "linux"))]
pub fn spawn(state: Arc<AppState>, shutdown: CancellationToken) {
    linux::spawn(state, shutdown);
}

#[cfg(not(all(feature = "camera", target_os = "linux")))]
pub fn spawn(_state: Arc<AppState>, _shutdown: CancellationToken) {}
