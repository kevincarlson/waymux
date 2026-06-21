//! Frame sources: producers of raw BGRA8 frames for the encode pipeline.
//!
//! [`FrameSource`] is the seam two backends implement: [`WaylandScreencopySource`]
//! captures a real compositor via `wlr-screencopy`, and [`TestPatternSource`]
//! emits an animated synthetic image so the pipeline runs without a compositor.

mod test_pattern;
mod wayland;

use std::future::Future;

use bytes::Bytes;
use tokio::sync::mpsc;
use waymux_proto::DisplayInfoMsg;

use crate::error::BridgeError;

pub use test_pattern::TestPatternSource;
pub use wayland::WaylandScreencopySource;

/// Bytes per pixel in a BGRA8 frame.
pub const BYTES_PER_PIXEL: u32 = 4;

/// An uncompressed BGRA8 frame captured from a source.
///
/// `pixels` is row-major BGRA8 with `stride` bytes per row; `stride` may exceed
/// `width * 4` when the source pads rows.
#[derive(Debug, Clone)]
pub struct RawFrame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Row stride in bytes (`>= width * 4`).
    pub stride: u32,
    /// Row-major BGRA8 pixel data.
    pub pixels: Bytes,
}

impl RawFrame {
    /// Creates a frame from its geometry and pixel buffer.
    #[must_use]
    pub fn new(width: u32, height: u32, stride: u32, pixels: Bytes) -> Self {
        Self {
            width,
            height,
            stride,
            pixels,
        }
    }

    /// The number of bytes one tightly-packed row occupies (`width * 4`).
    #[must_use]
    pub fn packed_row_len(&self) -> usize {
        self.width as usize * BYTES_PER_PIXEL as usize
    }
}

/// A producer of [`RawFrame`]s.
///
/// Implementors drive frames into the provided channel from [`run`]. The
/// pipeline pulls from the other end, encodes, and fans out to clients.
///
/// [`run`]: FrameSource::run
pub trait FrameSource {
    /// Display geometry advertised to clients on connect.
    fn display_info(&self) -> DisplayInfoMsg;

    /// Runs the capture loop, sending frames to `tx` until the receiver is
    /// dropped (clean shutdown) or an error occurs.
    fn run(
        self,
        tx: mpsc::Sender<RawFrame>,
    ) -> impl Future<Output = Result<(), BridgeError>> + Send
    where
        Self: Sized;
}
