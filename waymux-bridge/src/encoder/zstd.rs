//! Zstd-compressed BGRA8 encoder (the production default, per ADR-002).

use bytes::Bytes;
use waymux_proto::FrameEncoding;

use super::{FrameEncoder, pack_rows};
use crate::error::EncoderError;
use crate::source::RawFrame;

/// Encoder that tightly packs a frame and compresses it with zstd.
#[derive(Debug, Clone, Copy)]
pub struct ZstdBgra8Encoder {
    level: i32,
}

impl ZstdBgra8Encoder {
    /// Creates an encoder using zstd compression `level` (1–22).
    #[must_use]
    pub fn new(level: i32) -> Self {
        Self { level }
    }
}

impl FrameEncoder for ZstdBgra8Encoder {
    fn encoding(&self) -> FrameEncoding {
        FrameEncoding::ZstdBgra8
    }

    fn encode(&mut self, frame: &RawFrame) -> Result<Bytes, EncoderError> {
        let packed = pack_rows(frame)?;
        let compressed =
            zstd::encode_all(packed.as_ref(), self.level).map_err(EncoderError::Zstd)?;
        Ok(Bytes::from(compressed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_zstd() {
        let pixels = Bytes::from(vec![0x42u8; 16 * 16 * 4]);
        let frame = RawFrame::new(16, 16, 16 * 4, pixels.clone());
        let mut enc = ZstdBgra8Encoder::new(3);
        assert_eq!(enc.encoding(), FrameEncoding::ZstdBgra8);
        let compressed = enc.encode(&frame).unwrap();
        let decoded = zstd::decode_all(compressed.as_ref()).unwrap();
        assert_eq!(decoded, pixels.as_ref());
    }

    #[test]
    fn compresses_uniform_frames() {
        let frame = RawFrame::new(64, 64, 64 * 4, Bytes::from(vec![0u8; 64 * 64 * 4]));
        let compressed = ZstdBgra8Encoder::new(3).encode(&frame).unwrap();
        assert!(
            compressed.len() < 64 * 64 * 4,
            "uniform frame should shrink"
        );
    }
}
