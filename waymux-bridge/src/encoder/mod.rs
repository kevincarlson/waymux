// SPDX-License-Identifier: Apache-2.0
//! Frame encoding pipeline: [`FrameEncoder`] trait and implementations.

mod raw;
mod zstd_enc;

pub use raw::RawBgra8Encoder;
pub use zstd_enc::ZstdBgra8Encoder;

use bytes::Bytes;
use waymux_proto::FrameEncoding;
use crate::config::{Config, EncodingChoice};
use crate::error::EncoderError;

/// Encodes a raw BGRA8 frame buffer into a [`bytes::Bytes`] payload.
///
/// Implementations must be stateless with respect to frame content —
/// each call to [`FrameEncoder::encode`] is independent.
pub trait FrameEncoder: Send + Sync + 'static {
    /// The [`FrameEncoding`] variant this encoder produces.
    fn encoding(&self) -> FrameEncoding;

    /// Encode `raw_bgra8` (width × height × 4 bytes, row-major) into
    /// an encoded payload ready for transmission over WFP.
    fn encode(&self, raw_bgra8: &[u8]) -> Result<Bytes, EncoderError>;
}

/// Construct a boxed [`FrameEncoder`] from a [`Config`].
pub fn from_config(config: &Config) -> Box<dyn FrameEncoder> {
    match config.encoding {
        EncodingChoice::Raw => Box::new(RawBgra8Encoder),
        EncodingChoice::Zstd => Box::new(ZstdBgra8Encoder { level: config.zstd_level }),
    }
}
