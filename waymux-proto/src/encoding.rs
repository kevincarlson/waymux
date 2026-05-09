// SPDX-License-Identifier: Apache-2.0

//! Frame encoding identifier used in WFP frame messages.

use crate::error::CodecError;

/// Identifies the encoding format of a frame payload transmitted over WFP.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameEncoding {
    /// Uncompressed BGRA8, 4 bytes per pixel, row-major.
    RawBgra8 = 0x00,
    /// Zstd-compressed BGRA8.
    ZstdBgra8 = 0x01,
    /// H.264 Annex B bitstream (reserved for future use; not implemented in M1–M5).
    H264AnnexB = 0x02,
}

impl TryFrom<u8> for FrameEncoding {
    type Error = CodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(FrameEncoding::RawBgra8),
            0x01 => Ok(FrameEncoding::ZstdBgra8),
            0x02 => Ok(FrameEncoding::H264AnnexB),
            other => Err(CodecError::UnknownEncoding(other)),
        }
    }
}

impl From<FrameEncoding> for u8 {
    fn from(encoding: FrameEncoding) -> u8 {
        encoding as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all_variants() -> Result<(), Box<dyn std::error::Error>> {
        let variants = [FrameEncoding::RawBgra8, FrameEncoding::ZstdBgra8, FrameEncoding::H264AnnexB];
        for &encoding in &variants {
            let byte = u8::from(encoding);
            let decoded = FrameEncoding::try_from(byte)?;
            assert_eq!(decoded, encoding);
        }
        Ok(())
    }

    #[test]
    fn unknown_byte_returns_error() {
        let result = FrameEncoding::try_from(0xFF);
        assert!(matches!(result, Err(CodecError::UnknownEncoding(0xFF))));
    }
}
