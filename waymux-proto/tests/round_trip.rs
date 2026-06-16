//! Encode → decode round-trip coverage for every WFP and WIP message variant.

use bytes::{Bytes, BytesMut};
use waymux_proto::*;

fn wfp_round_trip(msg: WfpMessage) {
    let mut buf = BytesMut::new();
    encode_wfp(&msg, &mut buf).expect("encode should succeed");
    let decoded = decode_wfp(&mut buf)
        .expect("decode should not error")
        .expect("a complete frame should be available");
    assert_eq!(decoded, msg);
    assert!(buf.is_empty(), "buffer should be fully consumed");
}

fn wip_round_trip(msg: WipMessage) {
    let mut buf = BytesMut::new();
    encode_wip(&msg, &mut buf).expect("encode should succeed");
    let decoded = decode_wip(&mut buf)
        .expect("decode should not error")
        .expect("a complete frame should be available");
    assert_eq!(decoded, msg);
    assert!(buf.is_empty(), "buffer should be fully consumed");
}

#[test]
fn wfp_all_variants_round_trip() {
    wfp_round_trip(WfpMessage::FrameFull(FrameFullMsg {
        width: 1920,
        height: 1080,
        encoding: FrameEncoding::RawBgra8,
        data: Bytes::from_static(&[1, 2, 3, 4, 5, 6, 7, 8]),
    }));
    wfp_round_trip(WfpMessage::FrameFull(FrameFullMsg {
        width: 0,
        height: 0,
        encoding: FrameEncoding::ZstdBgra8,
        data: Bytes::new(),
    }));
    wfp_round_trip(WfpMessage::FrameDamage(FrameDamageMsg {
        encoding: FrameEncoding::ZstdBgra8,
        regions: vec![
            DamageRegion {
                x: 0,
                y: 0,
                width: 64,
                height: 64,
            },
            DamageRegion {
                x: 100,
                y: 200,
                width: 10,
                height: 20,
            },
        ],
        data: Bytes::from_static(&[9, 9, 9]),
    }));
    wfp_round_trip(WfpMessage::FrameDamage(FrameDamageMsg {
        encoding: FrameEncoding::RawBgra8,
        regions: Vec::new(),
        data: Bytes::new(),
    }));
    wfp_round_trip(WfpMessage::DisplayInfo(DisplayInfoMsg {
        width: 2560,
        height: 1440,
        scale_factor: 1.5,
        refresh_hz: 59.94,
    }));
    wfp_round_trip(WfpMessage::Ping { sequence: u64::MAX });
    wfp_round_trip(WfpMessage::Disconnect {
        reason: DisconnectReason::ServerShutdown,
    });
    wfp_round_trip(WfpMessage::Disconnect {
        reason: DisconnectReason::ProtocolError,
    });
    wfp_round_trip(WfpMessage::Disconnect {
        reason: DisconnectReason::CompositorLost,
    });
}

#[test]
fn wip_all_variants_round_trip() {
    wip_round_trip(WipMessage::PointerMotion(PointerMotionMsg {
        x: 12.5,
        y: -3.0,
        time_ms: 100,
    }));
    wip_round_trip(WipMessage::PointerButton(PointerButtonMsg {
        button: 0x110,
        state: ButtonState::Pressed,
        time_ms: 101,
    }));
    wip_round_trip(WipMessage::PointerButton(PointerButtonMsg {
        button: 0x111,
        state: ButtonState::Released,
        time_ms: 102,
    }));
    wip_round_trip(WipMessage::PointerAxis(PointerAxisMsg {
        axis: PointerAxis::Vertical,
        value: -1.0,
        time_ms: 103,
    }));
    wip_round_trip(WipMessage::PointerAxis(PointerAxisMsg {
        axis: PointerAxis::Horizontal,
        value: 2.5,
        time_ms: 104,
    }));
    wip_round_trip(WipMessage::TouchDown(TouchPointMsg {
        id: 1,
        x: 5.0,
        y: 6.0,
        time_ms: 105,
    }));
    wip_round_trip(WipMessage::TouchMotion(TouchPointMsg {
        id: 1,
        x: 7.0,
        y: 8.0,
        time_ms: 106,
    }));
    wip_round_trip(WipMessage::TouchUp {
        id: 1,
        time_ms: 107,
    });
    wip_round_trip(WipMessage::StylusDown(StylusMsg {
        x: 1.0,
        y: 2.0,
        pressure: 0.5,
        tilt_x: 0.1,
        tilt_y: -0.2,
        time_ms: 108,
    }));
    wip_round_trip(WipMessage::StylusMotion(StylusMsg {
        x: 1.5,
        y: 2.5,
        pressure: 0.9,
        tilt_x: 0.0,
        tilt_y: 0.0,
        time_ms: 109,
    }));
    wip_round_trip(WipMessage::StylusUp { time_ms: 110 });
    wip_round_trip(WipMessage::KeyDown(KeyMsg {
        keycode: 30,
        modifiers: 0x4,
        time_ms: 111,
    }));
    wip_round_trip(WipMessage::KeyUp(KeyMsg {
        keycode: 30,
        modifiers: 0,
        time_ms: 112,
    }));
    wip_round_trip(WipMessage::Pong { sequence: 42 });
}

#[test]
fn unknown_wfp_discriminant_is_rejected() {
    // Frame: length prefix (1) + a single unknown type byte.
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&[0x55]);
    assert_eq!(decode_wfp(&mut buf), Err(CodecError::UnknownWfpType(0x55)));
}

#[test]
fn unknown_wip_discriminant_is_rejected() {
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&[0x55]);
    assert_eq!(decode_wip(&mut buf), Err(CodecError::UnknownWipType(0x55)));
}
