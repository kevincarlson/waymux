// SPDX-License-Identifier: Apache-2.0

//! Tests that verify `decode_wfp` returns `Ok(None)` for every partial prefix
//! of an encoded frame, and correctly decodes the full frame once all bytes
//! are present.

use bytes::{Bytes, BytesMut};
use waymux_proto::{
    decode_wfp, encode_wfp, DamageRegion, DisplayInfoMsg, FrameDamageMsg, FrameFullMsg,
    FrameEncoding, WfpMessage,
};

/// Encodes `msg` and returns a fully-owned `Vec<u8>` of the wire bytes.
fn encode(msg: &WfpMessage) -> Vec<u8> {
    let mut buf = BytesMut::new();
    encode_wfp(msg, &mut buf).unwrap();
    buf.to_vec()
}

/// For each split point 1..len-1, feeds the partial bytes to `decode_wfp` and
/// asserts `Ok(None)`, then feeds the complete frame and asserts it decodes to
/// the expected message.
fn assert_partial_then_full(expected: &WfpMessage) {
    let wire = encode(expected);
    let full_len = wire.len();

    for split in 1..full_len {
        let mut partial = BytesMut::from(&wire[..split]);
        let result = decode_wfp(&mut partial).expect("should not error on partial");
        assert!(
            result.is_none(),
            "expected Ok(None) for {split}/{full_len} bytes, got Some(..)"
        );
        // Buffer must be unchanged (decoder must not consume partial frames)
        assert_eq!(partial.len(), split, "decoder must not consume bytes from partial frame");
    }

    // Full frame must decode correctly
    let mut full = BytesMut::from(&wire[..]);
    let decoded = decode_wfp(&mut full).unwrap().expect("full frame must decode");
    assert_eq!(&decoded, expected);
    assert_eq!(full.len(), 0, "decoder must consume exactly the frame bytes");
}

#[test]
fn partial_frame_full_raw_bgra8() {
    let msg = WfpMessage::FrameFull(FrameFullMsg {
        width: 320,
        height: 240,
        encoding: FrameEncoding::RawBgra8,
        data: Bytes::from(vec![0xAA; 16]),
    });
    assert_partial_then_full(&msg);
}

#[test]
fn partial_frame_damage_two_regions() {
    let msg = WfpMessage::FrameDamage(FrameDamageMsg {
        encoding: FrameEncoding::ZstdBgra8,
        regions: vec![
            DamageRegion { x: 0, y: 0, width: 64, height: 64 },
            DamageRegion { x: 128, y: 128, width: 32, height: 32 },
        ],
        data: Bytes::from(vec![0xBB; 8]),
    });
    assert_partial_then_full(&msg);
}

#[test]
fn partial_display_info() {
    let msg = WfpMessage::DisplayInfo(DisplayInfoMsg {
        width: 1920,
        height: 1080,
        scale_factor: 1.5,
        refresh_hz: 120.0,
    });
    assert_partial_then_full(&msg);
}

#[test]
fn partial_ping() {
    let msg = WfpMessage::Ping { sequence: 0xCAFE_BABE_0000_1111 };
    assert_partial_then_full(&msg);
}

#[test]
fn partial_disconnect() {
    let msg = WfpMessage::Disconnect { reason: waymux_proto::DisconnectReason::CompositorLost };
    assert_partial_then_full(&msg);
}

/// Verifies that two consecutive frames in the same buffer are both decoded
/// and that no bytes bleed between frames.
#[test]
fn two_consecutive_frames() {
    let msg1 = WfpMessage::Ping { sequence: 1 };
    let msg2 = WfpMessage::Ping { sequence: 2 };
    let mut buf = BytesMut::new();
    encode_wfp(&msg1, &mut buf).unwrap();
    encode_wfp(&msg2, &mut buf).unwrap();

    let decoded1 = decode_wfp(&mut buf).unwrap().expect("frame 1");
    let decoded2 = decode_wfp(&mut buf).unwrap().expect("frame 2");
    assert_eq!(decoded1, msg1);
    assert_eq!(decoded2, msg2);
    assert_eq!(buf.len(), 0);
}
