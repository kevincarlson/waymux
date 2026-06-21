//! An animated synthetic [`FrameSource`] used for development and testing.
//!
//! It renders a moving diagonal BGRA8 gradient at a fixed resolution and frame
//! rate, so the bridge can stream real frames without a Wayland compositor.

use std::time::Duration;

use bytes::{Bytes, BytesMut};
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};
use waymux_proto::DisplayInfoMsg;

use super::{BYTES_PER_PIXEL, FrameSource, RawFrame};
use crate::error::BridgeError;

/// Source that emits an animated gradient at `width`x`height` and `max_fps`.
#[derive(Debug, Clone)]
pub struct TestPatternSource {
    width: u32,
    height: u32,
    max_fps: u32,
}

impl TestPatternSource {
    /// Creates a source. `max_fps` is clamped to at least 1.
    #[must_use]
    pub fn new(width: u32, height: u32, max_fps: u32) -> Self {
        Self {
            width,
            height,
            max_fps: max_fps.max(1),
        }
    }

    /// Renders frame number `n` into a fresh tightly-packed BGRA8 buffer.
    #[must_use]
    pub fn render(&self, n: u64) -> Bytes {
        let stride = self.width as usize * BYTES_PER_PIXEL as usize;
        let mut buf = BytesMut::with_capacity(stride * self.height as usize);
        let shift = (n & 0xff) as u32;
        for y in 0..self.height {
            for x in 0..self.width {
                // A diagonal gradient that scrolls with the frame counter.
                let b = ((x + shift) & 0xff) as u8;
                let g = ((y + shift) & 0xff) as u8;
                let r = ((x + y + shift) & 0xff) as u8;
                buf.extend_from_slice(&[b, g, r, 0xff]);
            }
        }
        buf.freeze()
    }
}

impl FrameSource for TestPatternSource {
    fn display_info(&self) -> DisplayInfoMsg {
        DisplayInfoMsg {
            width: self.width,
            height: self.height,
            scale_factor: 1.0,
            refresh_hz: self.max_fps as f32,
        }
    }

    async fn run(self, tx: mpsc::Sender<RawFrame>) -> Result<(), BridgeError> {
        let period = Duration::from_secs_f64(1.0 / f64::from(self.max_fps));
        let mut ticker = interval(period);
        // If the consumer stalls we skip missed ticks rather than bursting.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let stride = self.width * BYTES_PER_PIXEL;

        let mut n: u64 = 0;
        loop {
            ticker.tick().await;
            let frame = RawFrame::new(self.width, self.height, stride, self.render(n));
            // A closed channel means the pipeline shut down: stop cleanly.
            if tx.send(frame).await.is_err() {
                return Ok(());
            }
            n = n.wrapping_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_produces_tightly_packed_bgra8() {
        let src = TestPatternSource::new(4, 3, 60);
        let frame = src.render(0);
        assert_eq!(frame.len(), 4 * 3 * 4);
        // Alpha channel of the first pixel is opaque.
        assert_eq!(frame[3], 0xff);
    }

    #[test]
    fn animation_shifts_between_frames() {
        let src = TestPatternSource::new(8, 8, 60);
        assert_ne!(src.render(0), src.render(1));
    }

    #[test]
    fn display_info_reflects_geometry() {
        let info = TestPatternSource::new(800, 600, 30).display_info();
        assert_eq!(info.width, 800);
        assert_eq!(info.height, 600);
        assert_eq!(info.refresh_hz, 30.0);
    }
}
