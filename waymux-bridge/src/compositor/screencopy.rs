// SPDX-License-Identifier: Apache-2.0
//! Screencopy state and Wayland frame-capture dispatch.

use std::fs::{File, OpenOptions};
use std::io::{Read as _, Seek as _, SeekFrom};
use std::os::fd::AsFd as _;

use bytes::{Bytes, BytesMut};
use tokio::sync::mpsc;
use wayland_client::protocol::wl_buffer;
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};
use wayland_client::protocol::wl_output::WlOutput;

use waymux_proto::{codec::encode_wfp, wfp::FrameFullMsg, WfpMessage};

use crate::encoder::FrameEncoder;
use super::BridgeState;

/// Dimensions of the current pending screencopy frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameDimensions {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Row stride in bytes.
    pub stride: u32,
}

/// Tracks state for a single in-flight screencopy capture.
pub struct ScreencopyState {
    /// Dimensions of the pending frame, populated on the `buffer` event.
    pub pending_dims: Option<FrameDimensions>,
    /// Channel for delivering encoded WFP frames to the tokio pipeline.
    pub frame_tx: mpsc::Sender<Bytes>,
    /// The frame encoder to apply before transmission.
    pub encoder: Box<dyn FrameEncoder>,
    /// Pixel readback buffer sized to the largest frame seen so far.
    pub(super) buf: Vec<u8>,
    /// Anonymous shared-memory file that the compositor writes pixels into.
    /// Kept alive so the wl_shm_pool fd remains valid until `Ready`.
    shm_file: Option<File>,
    /// `wl_buffer` object handed to the compositor for the current capture.
    pending_buffer: Option<wl_buffer::WlBuffer>,
    /// Monotonically increasing frame sequence number used for unique tmp names.
    frame_seq: u64,
}

impl ScreencopyState {
    /// Create a new [`ScreencopyState`].
    pub fn new(frame_tx: mpsc::Sender<Bytes>, encoder: Box<dyn FrameEncoder>) -> Self {
        ScreencopyState {
            pending_dims: None,
            frame_tx,
            encoder,
            buf: Vec::new(),
            shm_file: None,
            pending_buffer: None,
            frame_seq: 0,
        }
    }

    /// Request a new screencopy frame from the compositor.
    ///
    /// Call this once after the initial global binding roundtrip, and again
    /// after each `ready` or `failed` event to keep the capture loop running.
    pub fn request_frame<D>(
        manager: &ZwlrScreencopyManagerV1,
        output: &WlOutput,
        qh: &QueueHandle<D>,
    ) where
        D: Dispatch<ZwlrScreencopyFrameV1, ()> + 'static,
    {
        // overlay_cursor = 0: do not composite cursor onto the frame.
        manager.capture_output(0, output, qh, ());
    }

    /// Create an anonymous backing file of `size` bytes for a wl_shm_pool.
    ///
    /// Uses the create-and-immediately-unlink pattern: the file disappears
    /// from the filesystem the moment it is opened but remains accessible
    /// via the returned [`File`] handle until it is dropped.
    fn make_shm_file(size: usize, seq: u64) -> std::io::Result<File> {
        let path = std::env::temp_dir()
            .join(format!(".waymux-shm-{}-{seq}", std::process::id()));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;
        // Unlink before truncate so the path is gone if we crash mid-way.
        let _ = std::fs::remove_file(&path);
        file.set_len(size as u64)?;
        Ok(file)
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for BridgeState {
    fn event(
        state: &mut Self,
        frame: &ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer { format, width, height, stride } => {
                // Accept only 4-bytes-per-pixel formats (stride must equal width × 4).
                let fmt = match format {
                    WEnum::Value(f) => f,
                    WEnum::Unknown(n) => {
                        tracing::debug!("screencopy: unknown shm format {n:#x}, skipping");
                        return;
                    }
                };
                if stride != width * 4 {
                    tracing::debug!("screencopy: skipping non-4bpp format {fmt:?}");
                    return;
                }

                let size = (stride * height) as usize;
                if size == 0 {
                    tracing::warn!("screencopy: zero-size frame, destroying");
                    frame.destroy();
                    return;
                }

                let Some(shm) = state.shm.as_ref() else {
                    tracing::error!("screencopy: wl_shm not bound; cannot allocate buffer");
                    frame.destroy();
                    return;
                };

                state.screencopy.pending_dims =
                    Some(FrameDimensions { width, height, stride });
                if state.screencopy.buf.len() < size {
                    state.screencopy.buf.resize(size, 0u8);
                }

                state.screencopy.frame_seq += 1;
                let seq = state.screencopy.frame_seq;

                let file = match ScreencopyState::make_shm_file(size, seq) {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::error!("screencopy: shm file creation failed: {e}");
                        frame.destroy();
                        return;
                    }
                };

                // Create the wl_shm_pool backed by the anonymous file.
                let pool = shm.create_pool(file.as_fd(), size as i32, qh, ());
                let wl_buf = pool.create_buffer(
                    0,
                    width as i32,
                    height as i32,
                    stride as i32,
                    fmt,
                    qh,
                    (),
                );
                // The pool is reference-counted by Wayland; destroy after buffer creation.
                pool.destroy();

                // Tell the compositor to write the current frame into our buffer.
                frame.copy(&wl_buf);
                tracing::debug!(width, height, stride, "screencopy: copy issued");

                state.screencopy.shm_file = Some(file);
                state.screencopy.pending_buffer = Some(wl_buf);
            }

            zwlr_screencopy_frame_v1::Event::BufferDone => {
                // All buffer-type advertisements sent (v3+ only).
                // Copy was already issued in the Buffer handler.
                tracing::trace!("screencopy: buffer_done");
            }

            zwlr_screencopy_frame_v1::Event::Flags { flags } => {
                let _ = flags;
            }

            zwlr_screencopy_frame_v1::Event::Damage { x, y, width, height } => {
                tracing::trace!(x, y, width, height, "screencopy: damage hint");
            }

            zwlr_screencopy_frame_v1::Event::Ready { tv_sec_hi, tv_sec_lo, tv_nsec } => {
                let _ = (tv_sec_hi, tv_sec_lo, tv_nsec);

                let dims = match state.screencopy.pending_dims {
                    Some(d) => d,
                    None => {
                        tracing::warn!("screencopy: ready without pending dimensions");
                        frame.destroy();
                        return;
                    }
                };
                let mut file = match state.screencopy.shm_file.take() {
                    Some(f) => f,
                    None => {
                        tracing::warn!("screencopy: ready without shm file");
                        frame.destroy();
                        return;
                    }
                };

                let size = (dims.stride * dims.height) as usize;

                // Read the compositor-written pixels back from the shared file.
                if let Err(e) = file.seek(SeekFrom::Start(0)) {
                    tracing::error!("screencopy: seek: {e}");
                    frame.destroy();
                    return;
                }
                let pixel_buf = &mut state.screencopy.buf[..size];
                if let Err(e) = file.read_exact(pixel_buf) {
                    tracing::error!("screencopy: read: {e}");
                    frame.destroy();
                    return;
                }
                drop(file); // release fd now that we have the pixel data

                // Encode the pixels and wrap in a WFP FrameFull message.
                let encoding = state.screencopy.encoder.encoding();
                let payload = match state.screencopy.encoder.encode(pixel_buf) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!("screencopy: encode: {e}");
                        frame.destroy();
                        return;
                    }
                };

                let wfp = WfpMessage::FrameFull(FrameFullMsg {
                    width: dims.width,
                    height: dims.height,
                    encoding,
                    data: payload,
                });
                let mut out = BytesMut::new();
                if let Err(e) = encode_wfp(&wfp, &mut out) {
                    tracing::error!("screencopy: encode_wfp: {e}");
                } else {
                    let _ = state.screencopy.frame_tx.try_send(out.freeze());
                    tracing::trace!(
                        width = dims.width,
                        height = dims.height,
                        "screencopy: frame sent to pipeline"
                    );
                }

                // Compositor is done with the buffer; destroy it before re-requesting.
                if let Some(buf) = state.screencopy.pending_buffer.take() {
                    buf.destroy();
                }
                frame.destroy();
                state.screencopy.pending_dims = None;

                // Immediately request the next frame to keep the capture loop running.
                if let (Some(mgr), Some(out)) =
                    (&state.screencopy_manager, &state.primary_output)
                {
                    ScreencopyState::request_frame(mgr, out, qh);
                }
            }

            zwlr_screencopy_frame_v1::Event::Failed => {
                tracing::error!("screencopy: compositor signalled frame capture failure");
                if let Some(buf) = state.screencopy.pending_buffer.take() {
                    buf.destroy();
                }
                state.screencopy.shm_file = None;
                state.screencopy.pending_dims = None;
                frame.destroy();
                // Do not re-request immediately to avoid a tight error loop.
            }

            _ => {}
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        _buffer: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Buffer is destroyed eagerly in the Ready handler; a late Release is a no-op.
        let _ = event;
    }
}
