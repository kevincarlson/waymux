//! Uncompressed BGRA8 passthrough encoder.

use bytes::Bytes;
use waymux_proto::FrameEncoding;

use super::{FrameEncoder, pack_rows};
use crate::error::EncoderError;
use crate::source::RawFrame;

/// Encoder that emits tightly-packed BGRA8 with no compression.
#[derive(Debug, Default, Clone, Copy)]
pub struct RawBgra8Encoder;

impl RawBgra8Encoder {
    /// Creates a new passthrough encoder.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl FrameEncoder for RawBgra8Encoder {
    fn encoding(&self) -> FrameEncoding {
        FrameEncoding::RawBgra8
    }

    fn encode(&mut self, frame: &RawFrame) -> Result<Bytes, EncoderError> {
        pack_rows(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_packed_pixels_unchanged() {
        let pixels = Bytes::from(vec![0xabu8; 3 * 2 * 4]);
        let frame = RawFrame::new(3, 2, 12, pixels.clone());
        let mut enc = RawBgra8Encoder::new();
        assert_eq!(enc.encoding(), FrameEncoding::RawBgra8);
        assert_eq!(enc.encode(&frame).unwrap(), pixels);
    }
}
