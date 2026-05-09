// SPDX-License-Identifier: Apache-2.0

//! Property-based round-trip tests using `proptest`.
//!
//! Each test generates random valid field values for a message type, encodes
//! it, decodes it, and asserts equality. Normal f32 strategies are used
//! (excluding NaN and infinity) because the codec uses bit-exact LE encoding.

use bytes::BytesMut;
use proptest::prelude::*;
use waymux_proto::{
    decode_wfp, decode_wip, encode_wfp, encode_wip, DisplayInfoMsg, PointerMotionMsg, StylusMsg,
    WfpMessage, WipMessage,
};

proptest! {
    /// WFP Ping: random u64 sequence survives encode→decode.
    #[test]
    fn prop_wfp_ping_round_trip(sequence: u64) {
        let msg = WfpMessage::Ping { sequence };
        let mut buf = BytesMut::new();
        encode_wfp(&msg, &mut buf).unwrap();
        let decoded = decode_wfp(&mut buf).unwrap().expect("must decode");
        prop_assert_eq!(decoded, msg);
    }

    /// WFP DisplayInfo: random width/height and normal f32 scale/refresh.
    #[test]
    fn prop_wfp_display_info_round_trip(
        width in 1u32..=7680,
        height in 1u32..=4320,
        scale_factor in proptest::num::f32::NORMAL,
        refresh_hz in proptest::num::f32::NORMAL,
    ) {
        let msg = WfpMessage::DisplayInfo(DisplayInfoMsg {
            width,
            height,
            scale_factor,
            refresh_hz,
        });
        let mut buf = BytesMut::new();
        encode_wfp(&msg, &mut buf).unwrap();
        let decoded = decode_wfp(&mut buf).unwrap().expect("must decode");
        prop_assert_eq!(decoded, msg);
    }

    /// WIP PointerMotion: random normal f32 x/y and u32 time.
    #[test]
    fn prop_wip_pointer_motion_round_trip(
        x in proptest::num::f32::NORMAL,
        y in proptest::num::f32::NORMAL,
        time_ms: u32,
    ) {
        let msg = WipMessage::PointerMotion(PointerMotionMsg { x, y, time_ms });
        let mut buf = BytesMut::new();
        encode_wip(&msg, &mut buf).unwrap();
        let decoded = decode_wip(&mut buf).unwrap().expect("must decode");
        prop_assert_eq!(decoded, msg);
    }

    /// WIP StylusDown: all normal f32 fields and u32 time.
    #[test]
    fn prop_wip_stylus_down_round_trip(
        x in proptest::num::f32::NORMAL,
        y in proptest::num::f32::NORMAL,
        pressure in proptest::num::f32::NORMAL,
        tilt_x in proptest::num::f32::NORMAL,
        tilt_y in proptest::num::f32::NORMAL,
        time_ms: u32,
    ) {
        let msg = WipMessage::StylusDown(StylusMsg { x, y, pressure, tilt_x, tilt_y, time_ms });
        let mut buf = BytesMut::new();
        encode_wip(&msg, &mut buf).unwrap();
        let decoded = decode_wip(&mut buf).unwrap().expect("must decode");
        prop_assert_eq!(decoded, msg);
    }
}
