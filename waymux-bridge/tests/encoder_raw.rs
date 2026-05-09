// SPDX-License-Identifier: Apache-2.0
//! Integration tests for the raw BGRA8 passthrough encoder.

use waymux_bridge::encoder::{FrameEncoder, RawBgra8Encoder};
use waymux_proto::FrameEncoding;

/// A 4×4 BGRA8 frame (64 bytes) used as test input.
fn make_frame() -> Vec<u8> {
    (0u8..=63u8).collect()
}

#[test]
fn output_equals_input() {
    let encoder = RawBgra8Encoder;
    let frame = make_frame();
    let encoded = encoder.encode(&frame).expect("encode should succeed");
    assert_eq!(encoded.as_ref(), frame.as_slice(), "raw encoder must not transform data");
}

#[test]
fn encoding_variant() {
    let encoder = RawBgra8Encoder;
    assert_eq!(
        encoder.encoding(),
        FrameEncoding::RawBgra8,
        "raw encoder must report RawBgra8 variant"
    );
}
