//! Property-based round-trip tests: random field values must survive an
//! encode → decode cycle exactly. Float fields are drawn from finite ranges so
//! bit-exact equality holds (NaN would break `assert_eq`).

use bytes::{Bytes, BytesMut};
use proptest::prelude::*;
use waymux_proto::*;

fn check_wfp(msg: &WfpMessage) -> Result<(), TestCaseError> {
    let mut buf = BytesMut::new();
    encode_wfp(msg, &mut buf).expect("encode");
    let decoded = decode_wfp(&mut buf)
        .expect("decode")
        .expect("complete frame");
    prop_assert_eq!(&decoded, msg);
    prop_assert!(buf.is_empty());
    Ok(())
}

fn check_wip(msg: &WipMessage) -> Result<(), TestCaseError> {
    let mut buf = BytesMut::new();
    encode_wip(msg, &mut buf).expect("encode");
    let decoded = decode_wip(&mut buf)
        .expect("decode")
        .expect("complete frame");
    prop_assert_eq!(&decoded, msg);
    prop_assert!(buf.is_empty());
    Ok(())
}

const COORD: std::ops::Range<f32> = -100_000.0..100_000.0;

proptest! {
    #[test]
    fn frame_full_round_trips(
        width in any::<u32>(),
        height in any::<u32>(),
        enc in 0u8..=2,
        data in prop::collection::vec(any::<u8>(), 0..1024),
    ) {
        let msg = WfpMessage::FrameFull(FrameFullMsg {
            width,
            height,
            encoding: FrameEncoding::from_u8(enc).unwrap(),
            data: Bytes::from(data),
        });
        check_wfp(&msg)?;
    }

    #[test]
    fn frame_damage_round_trips(
        regions in prop::collection::vec(
            (any::<u32>(), any::<u32>(), any::<u32>(), any::<u32>()), 0..32),
        data in prop::collection::vec(any::<u8>(), 0..512),
    ) {
        let regions = regions
            .into_iter()
            .map(|(x, y, width, height)| DamageRegion { x, y, width, height })
            .collect();
        let msg = WfpMessage::FrameDamage(FrameDamageMsg {
            encoding: FrameEncoding::ZstdBgra8,
            regions,
            data: Bytes::from(data),
        });
        check_wfp(&msg)?;
    }

    #[test]
    fn display_info_round_trips(
        width in any::<u32>(),
        height in any::<u32>(),
        scale in 0.1f32..8.0,
        refresh in 1.0f32..360.0,
    ) {
        let msg = WfpMessage::DisplayInfo(DisplayInfoMsg {
            width,
            height,
            scale_factor: scale,
            refresh_hz: refresh,
        });
        check_wfp(&msg)?;
    }

    #[test]
    fn pointer_motion_round_trips(x in COORD, y in COORD, time_ms in any::<u32>()) {
        check_wip(&WipMessage::PointerMotion(PointerMotionMsg { x, y, time_ms }))?;
    }

    #[test]
    fn stylus_round_trips(
        x in COORD,
        y in COORD,
        pressure in 0.0f32..=1.0,
        tilt_x in -1.5f32..1.5,
        tilt_y in -1.5f32..1.5,
        time_ms in any::<u32>(),
    ) {
        let stylus = StylusMsg { x, y, pressure, tilt_x, tilt_y, time_ms };
        check_wip(&WipMessage::StylusDown(stylus))?;
        check_wip(&WipMessage::StylusMotion(stylus))?;
    }

    #[test]
    fn touch_round_trips(id in any::<u32>(), x in COORD, y in COORD, time_ms in any::<u32>()) {
        let point = TouchPointMsg { id, x, y, time_ms };
        check_wip(&WipMessage::TouchDown(point))?;
        check_wip(&WipMessage::TouchMotion(point))?;
    }

    #[test]
    fn key_round_trips(keycode in any::<u32>(), modifiers in any::<u32>(), time_ms in any::<u32>()) {
        let key = KeyMsg { keycode, modifiers, time_ms };
        check_wip(&WipMessage::KeyDown(key))?;
        check_wip(&WipMessage::KeyUp(key))?;
    }

    #[test]
    fn pong_round_trips(sequence in any::<u64>()) {
        check_wip(&WipMessage::Pong { sequence })?;
    }
}
