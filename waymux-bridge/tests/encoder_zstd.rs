// SPDX-License-Identifier: Apache-2.0
//! Integration tests for the Zstd-compressed BGRA8 encoder.

use std::io::Cursor;
use waymux_bridge::encoder::{FrameEncoder, ZstdBgra8Encoder};
use waymux_proto::FrameEncoding;

/// A highly compressible 64×64 BGRA8 frame (16 384 bytes of zeroes).
fn make_compressible_frame() -> Vec<u8> {
    vec![0u8; 64 * 64 * 4]
}

#[test]
fn round_trip() {
    let encoder = ZstdBgra8Encoder { level: 3 };
    let frame = make_compressible_frame();
    let encoded = encoder.encode(&frame).expect("encode should succeed");
    let decoded = zstd::decode_all(Cursor::new(&encoded[..])).expect("decode should succeed");
    assert_eq!(decoded, frame, "decoded bytes must equal original frame");
}

#[test]
fn smaller_than_raw() {
    let encoder = ZstdBgra8Encoder { level: 3 };
    let frame = make_compressible_frame();
    let encoded = encoder.encode(&frame).expect("encode should succeed");
    assert!(
        encoded.len() < frame.len(),
        "zstd-compressed frame ({} bytes) must be smaller than raw ({} bytes)",
        encoded.len(),
        frame.len()
    );
}

#[test]
fn encoding_variant() {
    let encoder = ZstdBgra8Encoder { level: 3 };
    assert_eq!(
        encoder.encoding(),
        FrameEncoding::ZstdBgra8,
        "zstd encoder must report ZstdBgra8 variant"
    );
}
