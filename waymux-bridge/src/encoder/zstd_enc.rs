// SPDX-License-Identifier: Apache-2.0
//! Zstd-compressed BGRA8 encoder.

use std::io::Cursor;
use bytes::Bytes;
use waymux_proto::FrameEncoding;
use crate::encoder::FrameEncoder;
use crate::error::EncoderError;

/// Zstd-compressed BGRA8 frame encoder.
///
/// Compresses raw BGRA8 frame data with zstd at a configurable compression
/// level. The default level of `3` provides a good balance between speed and
/// compression ratio for real-time screen capture.
pub struct ZstdBgra8Encoder {
    /// Zstd compression level (1–22; 3 is the recommended default).
    pub level: i32,
}

impl FrameEncoder for ZstdBgra8Encoder {
    fn encoding(&self) -> FrameEncoding {
        FrameEncoding::ZstdBgra8
    }

    fn encode(&self, raw_bgra8: &[u8]) -> Result<Bytes, EncoderError> {
        let compressed = zstd::encode_all(Cursor::new(raw_bgra8), self.level)
            .map_err(|e| EncoderError::ZstdCompress(e.to_string()))?;
        Ok(Bytes::from(compressed))
    }
}
