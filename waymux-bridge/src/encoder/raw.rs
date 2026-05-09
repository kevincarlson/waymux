// SPDX-License-Identifier: Apache-2.0
//! Raw BGRA8 passthrough encoder.

use bytes::Bytes;
use waymux_proto::FrameEncoding;
use crate::encoder::FrameEncoder;
use crate::error::EncoderError;

/// Passthrough encoder: copies raw BGRA8 frame data unchanged.
///
/// Intended for debugging and benchmarking — it produces the largest possible
/// frames, so prefer [`super::ZstdBgra8Encoder`] in production.
pub struct RawBgra8Encoder;

impl FrameEncoder for RawBgra8Encoder {
    fn encoding(&self) -> FrameEncoding {
        FrameEncoding::RawBgra8
    }

    fn encode(&self, raw_bgra8: &[u8]) -> Result<Bytes, EncoderError> {
        // PERF: clone justified — raw encoder is passthrough; cost is acceptable for dev/debug use
        Ok(Bytes::copy_from_slice(raw_bgra8))
    }
}
