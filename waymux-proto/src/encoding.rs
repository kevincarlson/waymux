//! Frame payload encoding identifiers shared by WFP frame messages.

use crate::error::CodecError;

/// Identifies the encoding format of a frame payload.
///
/// The discriminant of each variant is its stable on-the-wire id and must not
/// change between protocol versions.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FrameEncoding {
    /// Uncompressed BGRA8, 4 bytes per pixel, row-major.
    RawBgra8 = 0x00,
    /// Zstd-compressed BGRA8.
    ZstdBgra8 = 0x01,
    /// H.264 Annex B bitstream (reserved for future use).
    H264AnnexB = 0x02,
}

impl FrameEncoding {
    /// Returns the wire discriminant byte for this encoding.
    #[must_use]
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Parses a wire discriminant byte into a [`FrameEncoding`].
    ///
    /// # Errors
    /// Returns [`CodecError::UnknownEncoding`] if `value` is not a known id.
    pub fn from_u8(value: u8) -> Result<Self, CodecError> {
        match value {
            0x00 => Ok(Self::RawBgra8),
            0x01 => Ok(Self::ZstdBgra8),
            0x02 => Ok(Self::H264AnnexB),
            other => Err(CodecError::UnknownEncoding(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_variant() {
        for enc in [
            FrameEncoding::RawBgra8,
            FrameEncoding::ZstdBgra8,
            FrameEncoding::H264AnnexB,
        ] {
            assert_eq!(FrameEncoding::from_u8(enc.to_u8()), Ok(enc));
        }
    }

    #[test]
    fn rejects_unknown_id() {
        assert_eq!(
            FrameEncoding::from_u8(0x7f),
            Err(CodecError::UnknownEncoding(0x7f))
        );
    }
}
