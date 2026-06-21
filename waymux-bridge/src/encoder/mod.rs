//! Frame encoders: turn a [`RawFrame`] into a WFP frame payload.
//!
//! Each encoder packs the (possibly padded) source rows into tightly-packed
//! BGRA8 and then applies its encoding. New tiers (e.g. H.264) implement
//! [`FrameEncoder`] without touching the pipeline.

mod raw;
mod zstd;

use bytes::Bytes;
use waymux_proto::FrameEncoding;

use crate::error::EncoderError;
use crate::source::{BYTES_PER_PIXEL, RawFrame};

pub use raw::RawBgra8Encoder;
pub use zstd::ZstdBgra8Encoder;

/// Encodes [`RawFrame`]s into WFP frame payloads.
pub trait FrameEncoder: Send {
    /// The [`FrameEncoding`] tag this encoder produces.
    fn encoding(&self) -> FrameEncoding;

    /// Encodes one frame into a payload suitable for `FrameFullMsg::data`.
    ///
    /// # Errors
    /// Returns [`EncoderError`] if the frame buffer is undersized or the
    /// underlying codec fails.
    fn encode(&mut self, frame: &RawFrame) -> Result<Bytes, EncoderError>;
}

/// Packs `frame` into tightly-packed BGRA8 rows (`width * 4` stride).
///
/// When the source is already tightly packed this returns a cheap `Bytes`
/// clone (a refcount bump); otherwise it copies row by row, dropping padding.
pub(crate) fn pack_rows(frame: &RawFrame) -> Result<Bytes, EncoderError> {
    let stride = frame.stride as usize;
    let height = frame.height as usize;
    let needed = stride.saturating_mul(height);
    if frame.pixels.len() < needed {
        return Err(EncoderError::ShortBuffer {
            expected: needed,
            actual: frame.pixels.len(),
            width: frame.width,
            height: frame.height,
            stride: frame.stride,
        });
    }

    let packed_row = frame.packed_row_len();
    if frame.stride == frame.width * BYTES_PER_PIXEL {
        // PERF: clone justified — Bytes::clone is an Arc refcount bump, not a copy.
        return Ok(frame.pixels.clone());
    }

    let mut out = bytes::BytesMut::with_capacity(packed_row * height);
    for row in 0..height {
        let start = row * stride;
        out.extend_from_slice(&frame.pixels[start..start + packed_row]);
    }
    Ok(out.freeze())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_rows_passes_through_tight_frames() {
        let pixels = Bytes::from(vec![7u8; 2 * 2 * 4]);
        let frame = RawFrame::new(2, 2, 8, pixels.clone());
        assert_eq!(pack_rows(&frame).unwrap(), pixels);
    }

    #[test]
    fn pack_rows_strips_row_padding() {
        // 2x2 frame, stride 12 (4 bytes padding per row).
        let mut padded = Vec::new();
        for row in 0..2u8 {
            padded.extend_from_slice(&[row; 8]); // 2 packed pixels
            padded.extend_from_slice(&[0xee; 4]); // padding
        }
        let frame = RawFrame::new(2, 2, 12, Bytes::from(padded));
        let packed = pack_rows(&frame).unwrap();
        assert_eq!(packed.len(), 2 * 2 * 4);
        assert_eq!(&packed[..8], &[0u8; 8]);
        assert_eq!(&packed[8..], &[1u8; 8]);
    }

    #[test]
    fn pack_rows_rejects_short_buffer() {
        let frame = RawFrame::new(4, 4, 16, Bytes::from(vec![0u8; 10]));
        assert!(matches!(
            pack_rows(&frame),
            Err(EncoderError::ShortBuffer { .. })
        ));
    }
}
