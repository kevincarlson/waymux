//! End-to-end coverage of the client core against a mock bridge over a real
//! Unix socket: frame decode + state snapshots + input forwarding.

use std::path::PathBuf;
use std::time::Duration;

use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use waymux_client::{ClientState, InputSerializer};
use waymux_proto::{
    DisplayInfoMsg, FrameEncoding, FrameFullMsg, WfpMessage, WipMessage, decode_wip, encode_wfp,
};

/// Accepts one client, sends `DisplayInfo` + `frame`, then returns the first
/// WIP message the client sends back.
async fn mock_bridge(listener: UnixListener, frame: WfpMessage) -> WipMessage {
    let (mut stream, _) = listener.accept().await.expect("accept");

    let mut out = BytesMut::new();
    let info = WfpMessage::DisplayInfo(DisplayInfoMsg {
        width: 2,
        height: 2,
        scale_factor: 1.0,
        refresh_hz: 60.0,
    });
    encode_wfp(&info, &mut out).expect("encode info");
    encode_wfp(&frame, &mut out).expect("encode frame");
    stream.write_all(&out).await.expect("write");

    let mut buf = BytesMut::new();
    let mut chunk = [0u8; 1024];
    loop {
        if let Some(msg) = decode_wip(&mut buf).expect("decode wip") {
            return msg;
        }
        let n = stream.read(&mut chunk).await.expect("read");
        assert_ne!(n, 0, "client closed before sending input");
        buf.extend_from_slice(&chunk[..n]);
    }
}

async fn await_frame(state: &ClientState) -> waymux_client::DecodedFrame {
    for _ in 0..200 {
        if let Some(frame) = state.latest_frame() {
            return frame;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("no frame decoded within timeout");
}

fn temp_socket(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("waymux-client-{tag}-{}.sock", std::process::id()))
}

#[tokio::test]
async fn decodes_zstd_frame_and_forwards_input() {
    let socket = temp_socket("e2e");
    let _ = std::fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket).expect("bind");

    // A 2x2 zstd-compressed frame of a known fill colour.
    let pixels = vec![0xABu8; 2 * 2 * 4];
    let compressed = zstd::encode_all(pixels.as_slice(), 3).expect("compress");
    let frame = WfpMessage::FrameFull(FrameFullMsg {
        width: 2,
        height: 2,
        encoding: FrameEncoding::ZstdBgra8,
        data: Bytes::from(compressed),
    });
    let server = tokio::spawn(mock_bridge(listener, frame));

    let state = ClientState::connect(&socket).await.expect("connect");

    let decoded = await_frame(&state).await;
    assert_eq!(decoded.width, 2);
    assert_eq!(decoded.height, 2);
    assert_eq!(decoded.pixels.as_ref(), pixels.as_slice());

    let info = state.display_info().expect("display info");
    assert_eq!(info.width, 2);

    // Forward a normalized pointer motion and confirm the server receives it.
    let mut serializer = InputSerializer::new();
    serializer.set_compositor_size(info.width, info.height);
    serializer.set_surface_size(4, 4); // surface twice the compositor size
    let msg = serializer.pointer_motion(2.0, 2.0, 42);
    state.send_input(msg).await.expect("send input");

    let received = server.await.expect("join server");
    match received {
        WipMessage::PointerMotion(m) => {
            // 2.0 surface px * (2 / 4) = 1.0 compositor px on each axis.
            assert_eq!((m.x, m.y), (1.0, 1.0));
            assert_eq!(m.time_ms, 42);
        }
        other => panic!("expected PointerMotion, got {other:?}"),
    }

    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn connect_fails_for_missing_socket() {
    let socket = temp_socket("missing");
    let _ = std::fs::remove_file(&socket);
    assert!(ClientState::connect(&socket).await.is_err());
}

#[tokio::test]
async fn client_replies_to_ping_with_pong() {
    let socket = temp_socket("ping");
    let _ = std::fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket).expect("bind");

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut out = BytesMut::new();
        encode_wfp(&WfpMessage::Ping { sequence: 99 }, &mut out).expect("encode ping");
        stream.write_all(&out).await.expect("write ping");

        let mut buf = BytesMut::new();
        let mut chunk = [0u8; 64];
        loop {
            if let Some(msg) = decode_wip(&mut buf).expect("decode") {
                return msg;
            }
            let n = stream.read(&mut chunk).await.expect("read");
            assert_ne!(n, 0, "client closed before replying to ping");
            buf.extend_from_slice(&chunk[..n]);
        }
    });

    let _state = ClientState::connect(&socket).await.expect("connect");
    let reply = tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("server timed out")
        .expect("join");
    assert_eq!(reply, WipMessage::Pong { sequence: 99 });

    drop(_state);
    let _ = std::fs::remove_file(&socket);
}
