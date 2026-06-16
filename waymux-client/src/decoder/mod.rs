//! WFP frame decoding: encoded payloads → tightly-packed BGRA8 pixels.
//!
//! The encoding is chosen per frame by the message, so decoding is a dispatch
//! function rather than a held encoder object (unlike the bridge's encoder).

mod raw;
mod zstd;

use bytes::Bytes;
use waymux_proto::{FrameEncoding, FrameFullMsg};

use crate::error::ClientError;

/// Bytes per pixel in a decoded BGRA8 frame.
pub const BYTES_PER_PIXEL: usize = 4;

/// A decoded frame ready to upload to a texture: row-major BGRA8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Tightly-packed BGRA8 pixels (`width * height * 4` bytes).
    pub pixels: Bytes,
}

/// Decodes a [`FrameFullMsg`] into a [`DecodedFrame`].
///
/// # Errors
/// Returns [`ClientError::UnsupportedEncoding`] for encodings the client does
/// not implement, [`ClientError::Zstd`] on decompression failure, and
/// [`ClientError::FrameSize`] if the pixel count disagrees with the geometry.
pub fn decode_full(msg: &FrameFullMsg) -> Result<DecodedFrame, ClientError> {
    let pixels = match msg.encoding {
        FrameEncoding::RawBgra8 => raw::decode(&msg.data),
        FrameEncoding::ZstdBgra8 => zstd::decode(&msg.data)?,
        FrameEncoding::H264AnnexB => return Err(ClientError::UnsupportedEncoding(msg.encoding)),
    };

    let expected = (msg.width as usize)
        .saturating_mul(msg.height as usize)
        .saturating_mul(BYTES_PER_PIXEL);
    if pixels.len() != expected {
        return Err(ClientError::FrameSize {
            expected,
            actual: pixels.len(),
        });
    }

    Ok(DecodedFrame {
        width: msg.width,
        height: msg.height,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_raw_frame() {
        let pixels = Bytes::from(vec![0x33u8; 2 * 2 * 4]);
        let msg = FrameFullMsg {
            width: 2,
            height: 2,
            encoding: FrameEncoding::RawBgra8,
            data: pixels.clone(),
        };
        let frame = decode_full(&msg).unwrap();
        assert_eq!(frame.width, 2);
        assert_eq!(frame.pixels, pixels);
    }

    #[test]
    fn rejects_wrong_size_payload() {
        let msg = FrameFullMsg {
            width: 4,
            height: 4,
            encoding: FrameEncoding::RawBgra8,
            data: Bytes::from(vec![0u8; 10]),
        };
        assert!(matches!(
            decode_full(&msg),
            Err(ClientError::FrameSize { .. })
        ));
    }

    #[test]
    fn rejects_unsupported_encoding() {
        let msg = FrameFullMsg {
            width: 1,
            height: 1,
            encoding: FrameEncoding::H264AnnexB,
            data: Bytes::new(),
        };
        assert!(matches!(
            decode_full(&msg),
            Err(ClientError::UnsupportedEncoding(_))
        ));
    }
}
