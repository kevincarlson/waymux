//! Verifies that the decoder returns `Ok(None)` until a full frame has arrived,
//! simulating byte-at-a-time delivery over a stream socket.

use bytes::{BufMut, Bytes, BytesMut};
use waymux_proto::*;

fn assert_streamed_decode(msg: WfpMessage) {
    let mut framed = BytesMut::new();
    encode_wfp(&msg, &mut framed).expect("encode");
    let framed = framed.freeze();

    let mut buf = BytesMut::new();
    for (idx, byte) in framed.iter().enumerate() {
        buf.put_u8(*byte);
        let result = decode_wfp(&mut buf).expect("decode must not error on partial input");
        if idx + 1 < framed.len() {
            assert!(
                result.is_none(),
                "frame should be incomplete after {} bytes",
                idx + 1
            );
        } else {
            assert_eq!(
                result,
                Some(msg.clone()),
                "final byte should complete the frame"
            );
        }
    }
    assert!(
        buf.is_empty(),
        "the completed frame should be fully consumed"
    );
}

#[test]
fn display_info_streams_byte_by_byte() {
    assert_streamed_decode(WfpMessage::DisplayInfo(DisplayInfoMsg {
        width: 1280,
        height: 720,
        scale_factor: 1.0,
        refresh_hz: 60.0,
    }));
}

#[test]
fn frame_full_with_payload_streams_byte_by_byte() {
    assert_streamed_decode(WfpMessage::FrameFull(FrameFullMsg {
        width: 4,
        height: 1,
        encoding: FrameEncoding::RawBgra8,
        data: Bytes::from_static(&[0xde, 0xad, 0xbe, 0xef, 0x00, 0x11, 0x22, 0x33]),
    }));
}

#[test]
fn split_across_two_chunks_is_reassembled() {
    let msg = WfpMessage::FrameFull(FrameFullMsg {
        width: 2,
        height: 2,
        encoding: FrameEncoding::ZstdBgra8,
        data: Bytes::from_static(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
    });
    let mut framed = BytesMut::new();
    encode_wfp(&msg, &mut framed).expect("encode");
    let framed = framed.freeze();

    let split = framed.len() / 2;
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&framed[..split]);
    assert_eq!(
        decode_wfp(&mut buf),
        Ok(None),
        "half a frame is not decodable"
    );
    buf.extend_from_slice(&framed[split..]);
    assert_eq!(decode_wfp(&mut buf), Ok(Some(msg)));
    assert!(buf.is_empty());
}
