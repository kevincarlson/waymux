// SPDX-License-Identifier: Apache-2.0
//! Screencopy state and Wayland frame-capture dispatch.

use bytes::Bytes;
use tokio::sync::mpsc;
use wayland_client::{protocol::wl_buffer, Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};
use wayland_client::protocol::wl_output::WlOutput;

use crate::encoder::FrameEncoder;
use crate::error::BridgeError;
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
    /// Channel for delivering encoded frames to the tokio pipeline.
    pub frame_tx: mpsc::Sender<Bytes>,
    /// The frame encoder to apply before transmission.
    pub encoder: Box<dyn FrameEncoder>,
    /// Pixel buffer allocated to hold one frame. Resized lazily.
    buf: Vec<u8>,
}

impl ScreencopyState {
    /// Create a new [`ScreencopyState`].
    pub fn new(frame_tx: mpsc::Sender<Bytes>, encoder: Box<dyn FrameEncoder>) -> Self {
        ScreencopyState {
            pending_dims: None,
            frame_tx,
            encoder,
            buf: Vec::new(),
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
                // Store dimensions. We accept whatever the compositor offers.
                tracing::debug!(width, height, stride, "screencopy: buffer event");
                state.screencopy.pending_dims = Some(FrameDimensions { width, height, stride });

                // Allocate/resize the pixel buffer.
                let size = (stride * height) as usize;
                if state.screencopy.buf.len() < size {
                    state.screencopy.buf.resize(size, 0u8);
                }

                // We cannot create a real wl_shm buffer here without a real
                // compositor providing the shm global. The copy request is
                // deferred until buffer_done (v3) or sent immediately (v1/v2).
                // For v1/v2 without buffer_done, send copy now if shm is ready.
                if let Some(shm_pool) = &state.shm_pool {
                    let _ = shm_pool; // borrow to suppress unused warning
                    // In production: create wl_buffer from pool and call frame.copy(&buf)
                    // Here we log and let buffer_done or a future event trigger the copy.
                }

                let _ = (frame, qh, format);
            }

            zwlr_screencopy_frame_v1::Event::BufferDone => {
                // All buffer types advertised; now issue the copy request.
                // In a real implementation: create wl_buffer from shm pool, call frame.copy().
                // For compilation correctness without a real compositor, log and continue.
                tracing::debug!("screencopy: buffer_done");
            }

            zwlr_screencopy_frame_v1::Event::Flags { flags } => {
                let _ = flags;
                tracing::debug!("screencopy: flags");
            }

            zwlr_screencopy_frame_v1::Event::Damage { x, y, width, height } => {
                tracing::debug!(x, y, width, height, "screencopy: damage");
            }

            zwlr_screencopy_frame_v1::Event::Ready { tv_sec_hi, tv_sec_lo, tv_nsec } => {
                let _ = (tv_sec_hi, tv_sec_lo, tv_nsec);
                tracing::debug!("screencopy: ready");

                let dims = match state.screencopy.pending_dims {
                    Some(d) => d,
                    None => {
                        tracing::warn!("screencopy: ready without known dimensions");
                        frame.destroy();
                        return;
                    }
                };

                // Encode the frame and forward to tokio pipeline.
                let slice = &state.screencopy.buf[..(dims.stride * dims.height) as usize];
                match state.screencopy.encoder.encode(slice) {
                    Ok(encoded) => {
                        let _ = state.screencopy.frame_tx.try_send(encoded);
                    }
                    Err(e) => {
                        tracing::error!("screencopy: encode error: {e}");
                    }
                }

                frame.destroy();
                state.screencopy.pending_dims = None;

                // Re-request next frame if we have the manager and output.
                if let (Some(mgr), Some(out)) = (&state.screencopy_manager, &state.primary_output)
                {
                    ScreencopyState::request_frame(mgr, out, qh);
                }
            }

            zwlr_screencopy_frame_v1::Event::Failed => {
                tracing::error!("screencopy: frame capture failed");
                frame.destroy();
                state.screencopy.pending_dims = None;

                // Retry on next timer tick — don't tight-loop on failure.
            }

            _ => {}
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        buffer: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            // The compositor is done with the buffer; it may be reused.
            buffer.destroy();
        }
    }
}

/// Wayland error from a BridgeError (used in event handlers).
#[allow(dead_code)]
pub(super) fn bridge_err_str(e: &BridgeError) -> String {
    e.to_string()
}
