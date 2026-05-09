// SPDX-License-Identifier: Apache-2.0

//! Round-trip encode→decode tests for every WFP and WIP message variant.

use bytes::{Bytes, BytesMut};
use waymux_proto::{
    decode_wfp, decode_wip, encode_wfp, encode_wip, ButtonState, DamageRegion, DisconnectReason,
    DisplayInfoMsg, FrameDamageMsg, FrameEncoding, FrameFullMsg, KeyMsg, PointerAxis,
    PointerAxisMsg, PointerButtonMsg, PointerMotionMsg, StylusMsg, TouchPointMsg, WfpMessage,
    WipMessage,
};

fn wfp_round_trip(msg: WfpMessage) -> WfpMessage {
    let mut buf = BytesMut::new();
    encode_wfp(&msg, &mut buf).unwrap();
    decode_wfp(&mut buf).unwrap().expect("should decode a complete frame")
}

fn wip_round_trip(msg: WipMessage) -> WipMessage {
    let mut buf = BytesMut::new();
    encode_wip(&msg, &mut buf).unwrap();
    decode_wip(&mut buf).unwrap().expect("should decode a complete frame")
}

// ── WFP ──────────────────────────────────────────────────────────────────────

#[test]
fn wfp_frame_full() {
    let msg = WfpMessage::FrameFull(FrameFullMsg {
        width: 1920,
        height: 1080,
        encoding: FrameEncoding::RawBgra8,
        data: Bytes::from(vec![0xDE, 0xAD, 0xBE, 0xEF]),
    });
    assert_eq!(wfp_round_trip(msg.clone()), msg);
}

#[test]
fn wfp_frame_damage_single_region() {
    let msg = WfpMessage::FrameDamage(FrameDamageMsg {
        encoding: FrameEncoding::ZstdBgra8,
        regions: vec![DamageRegion { x: 10, y: 20, width: 100, height: 50 }],
        data: Bytes::from_static(b"\xFF\x00\xFF\x00"),
    });
    assert_eq!(wfp_round_trip(msg.clone()), msg);
}

#[test]
fn wfp_frame_damage_multiple_regions() {
    let msg = WfpMessage::FrameDamage(FrameDamageMsg {
        encoding: FrameEncoding::RawBgra8,
        regions: vec![
            DamageRegion { x: 0, y: 0, width: 200, height: 200 },
            DamageRegion { x: 400, y: 300, width: 50, height: 50 },
        ],
        data: Bytes::from(vec![1, 2, 3]),
    });
    assert_eq!(wfp_round_trip(msg.clone()), msg);
}

#[test]
fn wfp_display_info() {
    let msg = WfpMessage::DisplayInfo(DisplayInfoMsg {
        width: 3840,
        height: 2160,
        scale_factor: 2.0,
        refresh_hz: 60.0,
    });
    assert_eq!(wfp_round_trip(msg.clone()), msg);
}

#[test]
fn wfp_ping() {
    let msg = WfpMessage::Ping { sequence: 0xDEAD_BEEF_1234_5678 };
    assert_eq!(wfp_round_trip(msg.clone()), msg);
}

#[test]
fn wfp_disconnect_all_reasons() {
    for reason in [
        DisconnectReason::ServerShutdown,
        DisconnectReason::ProtocolError,
        DisconnectReason::CompositorLost,
    ] {
        let msg = WfpMessage::Disconnect { reason };
        assert_eq!(wfp_round_trip(msg.clone()), msg);
    }
}

// ── WIP ──────────────────────────────────────────────────────────────────────

#[test]
fn wip_pointer_motion() {
    let msg = WipMessage::PointerMotion(PointerMotionMsg { x: 512.5, y: 384.0, time_ms: 1000 });
    assert_eq!(wip_round_trip(msg.clone()), msg);
}

#[test]
fn wip_pointer_button() {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        let msg = WipMessage::PointerButton(PointerButtonMsg {
            button: 0x110,
            state,
            time_ms: 2000,
        });
        assert_eq!(wip_round_trip(msg.clone()), msg);
    }
}

#[test]
fn wip_pointer_axis() {
    for axis in [PointerAxis::Vertical, PointerAxis::Horizontal] {
        let msg = WipMessage::PointerAxis(PointerAxisMsg { axis, value: 3.0, time_ms: 3000 });
        assert_eq!(wip_round_trip(msg.clone()), msg);
    }
}

#[test]
fn wip_touch_down() {
    let msg = WipMessage::TouchDown(TouchPointMsg { id: 1, x: 100.0, y: 200.0, time_ms: 4000 });
    assert_eq!(wip_round_trip(msg.clone()), msg);
}

#[test]
fn wip_touch_motion() {
    let msg =
        WipMessage::TouchMotion(TouchPointMsg { id: 1, x: 110.0, y: 210.0, time_ms: 4050 });
    assert_eq!(wip_round_trip(msg.clone()), msg);
}

#[test]
fn wip_touch_up() {
    let msg = WipMessage::TouchUp { id: 1, time_ms: 4100 };
    assert_eq!(wip_round_trip(msg.clone()), msg);
}

#[test]
fn wip_stylus_down() {
    let msg = WipMessage::StylusDown(StylusMsg {
        x: 300.0,
        y: 400.0,
        pressure: 0.75,
        tilt_x: -10.0,
        tilt_y: 5.0,
        time_ms: 5000,
    });
    assert_eq!(wip_round_trip(msg.clone()), msg);
}

#[test]
fn wip_stylus_motion() {
    let msg = WipMessage::StylusMotion(StylusMsg {
        x: 305.0,
        y: 402.0,
        pressure: 0.8,
        tilt_x: -11.0,
        tilt_y: 4.5,
        time_ms: 5010,
    });
    assert_eq!(wip_round_trip(msg.clone()), msg);
}

#[test]
fn wip_stylus_up() {
    let msg = WipMessage::StylusUp { time_ms: 5100 };
    assert_eq!(wip_round_trip(msg.clone()), msg);
}

#[test]
fn wip_key_down() {
    let msg = WipMessage::KeyDown(KeyMsg { keycode: 65, modifiers: 0, time_ms: 6000 });
    assert_eq!(wip_round_trip(msg.clone()), msg);
}

#[test]
fn wip_key_up() {
    let msg = WipMessage::KeyUp(KeyMsg { keycode: 65, modifiers: 0, time_ms: 6050 });
    assert_eq!(wip_round_trip(msg.clone()), msg);
}

#[test]
fn wip_pong() {
    let msg = WipMessage::Pong { sequence: 42 };
    assert_eq!(wip_round_trip(msg.clone()), msg);
}

// ── Wire format spot-checks ───────────────────────────────────────────────────

/// Verifies the exact byte layout of a WFP Ping frame.
///
/// Expected: `[09 00 00 00][10][78 56 34 12 00 00 00 00]`
#[test]
fn wfp_ping_exact_bytes() {
    let mut buf = BytesMut::new();
    encode_wfp(&WfpMessage::Ping { sequence: 0x0000_0000_1234_5678 }, &mut buf).unwrap();
    let expected: &[u8] = &[
        0x09, 0x00, 0x00, 0x00, // payload_len = 9
        0x10,                   // discriminant
        0x78, 0x56, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00, // sequence LE
    ];
    assert_eq!(buf.as_ref(), expected);
}

/// Verifies the exact byte layout of a WIP StylusDown frame.
///
/// Payload: `[0x07][x f32 LE][y f32 LE][pressure f32 LE][tilt_x f32 LE][tilt_y f32 LE][time_ms u32 LE]`
#[test]
fn wip_stylus_down_exact_bytes() {
    let msg = WipMessage::StylusDown(StylusMsg {
        x: 1.0_f32,
        y: 2.0_f32,
        pressure: 0.5_f32,
        tilt_x: 0.0_f32,
        tilt_y: 0.0_f32,
        time_ms: 0,
    });
    let mut buf = BytesMut::new();
    encode_wip(&msg, &mut buf).unwrap();
    assert_eq!(&buf[0..4], &[0x19, 0x00, 0x00, 0x00]); // payload_len = 25
    assert_eq!(buf[4], 0x07);
    assert_eq!(&buf[5..9], 1.0_f32.to_le_bytes());
    assert_eq!(&buf[9..13], 2.0_f32.to_le_bytes());
    assert_eq!(&buf[13..17], 0.5_f32.to_le_bytes());
    assert_eq!(&buf[17..21], 0.0_f32.to_le_bytes());
    assert_eq!(&buf[21..25], 0.0_f32.to_le_bytes());
    assert_eq!(&buf[25..29], 0u32.to_le_bytes());
}

/// Verifies the exact byte layout of a WFP FrameFull frame.
///
/// Payload: `[0x01][width u32 LE][height u32 LE][encoding u8][data_len u32 LE][data]`
#[test]
fn wfp_frame_full_exact_bytes() {
    let msg = WfpMessage::FrameFull(FrameFullMsg {
        width: 1,
        height: 2,
        encoding: FrameEncoding::RawBgra8,
        data: Bytes::from_static(b"\x01\x02"),
    });
    let mut buf = BytesMut::new();
    encode_wfp(&msg, &mut buf).unwrap();
    // payload_len = 1+4+4+1+4+2 = 16
    assert_eq!(&buf[0..4], &[0x10, 0x00, 0x00, 0x00]);
    assert_eq!(buf[4], 0x01);
    assert_eq!(&buf[5..9], 1u32.to_le_bytes());
    assert_eq!(&buf[9..13], 2u32.to_le_bytes());
    assert_eq!(buf[13], 0x00); // RawBgra8
    assert_eq!(&buf[14..18], 2u32.to_le_bytes());
    assert_eq!(&buf[18..20], b"\x01\x02");
}
