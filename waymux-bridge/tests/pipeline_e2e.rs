//! End-to-end coverage: a real Unix socket client connects to the server, the
//! pipeline runs a finite source, and the client decodes the WFP stream.

use std::time::Duration;

use bytes::{Bytes, BytesMut};
use tokio::io::AsyncReadExt;
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use waymux_bridge::encoder::RawBgra8Encoder;
use waymux_bridge::error::BridgeError;
use waymux_bridge::pipeline;
use waymux_bridge::server::Server;
use waymux_bridge::source::{FrameSource, RawFrame};
use waymux_proto::{DisplayInfoMsg, FrameEncoding, WfpMessage, decode_wfp};

/// A source that emits `frames` solid 2x2 frames then completes.
struct CountedSource {
    frames: u32,
}

impl FrameSource for CountedSource {
    fn display_info(&self) -> DisplayInfoMsg {
        DisplayInfoMsg {
            width: 2,
            height: 2,
            scale_factor: 1.0,
            refresh_hz: 60.0,
        }
    }

    async fn run(self, tx: mpsc::Sender<RawFrame>) -> Result<(), BridgeError> {
        for i in 0..self.frames {
            let fill = i as u8;
            let frame = RawFrame::new(2, 2, 8, Bytes::from(vec![fill; 16]));
            if tx.send(frame).await.is_err() {
                break;
            }
            // Yield so the consumer can drain between frames.
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        Ok(())
    }
}

/// Reads exactly one complete WFP message from `stream`, buffering as needed.
async fn read_message(stream: &mut UnixStream, buf: &mut BytesMut) -> WfpMessage {
    let mut chunk = [0u8; 1024];
    loop {
        if let Some(msg) = decode_wfp(buf).expect("decode") {
            return msg;
        }
        let n = stream.read(&mut chunk).await.expect("read");
        assert_ne!(n, 0, "server closed before a full message arrived");
        buf.extend_from_slice(&chunk[..n]);
    }
}

#[tokio::test]
async fn client_receives_display_info_then_frames() {
    let socket = std::env::temp_dir().join(format!("waymux-e2e-{}.sock", std::process::id()));
    let server = Server::bind(
        &socket,
        DisplayInfoMsg {
            width: 2,
            height: 2,
            scale_factor: 1.0,
            refresh_hz: 60.0,
        },
        3,
        None,
    )
    .expect("bind");
    let handle = server.handle();
    let accept = tokio::spawn(server.run_accept());

    let mut client = UnixStream::connect(&socket).await.expect("connect");
    // Wait until the server has registered the client before broadcasting.
    while handle.client_count().await == 0 {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    let pipeline = tokio::spawn(pipeline::run(
        CountedSource { frames: 3 },
        Box::new(RawBgra8Encoder::new()),
        handle.clone(),
        2,
    ));

    let mut buf = BytesMut::new();

    // First message must be DisplayInfo.
    match read_message(&mut client, &mut buf).await {
        WfpMessage::DisplayInfo(info) => assert_eq!(info.width, 2),
        other => panic!("expected DisplayInfo first, got {other:?}"),
    }

    // Then at least one frame (backpressure may coalesce the three).
    match read_message(&mut client, &mut buf).await {
        WfpMessage::FrameFull(frame) => {
            assert_eq!(frame.encoding, FrameEncoding::RawBgra8);
            assert_eq!(frame.width, 2);
            assert_eq!(frame.data.len(), 16);
        }
        other => panic!("expected FrameFull, got {other:?}"),
    }

    pipeline.await.expect("join").expect("pipeline ok");
    accept.abort();
    let _ = std::fs::remove_file(&socket);
}

#[tokio::test]
async fn server_reports_connect_and_disconnect() {
    let socket = std::env::temp_dir().join(format!("waymux-conn-{}.sock", std::process::id()));
    let server = Server::bind(
        &socket,
        DisplayInfoMsg {
            width: 1,
            height: 1,
            scale_factor: 1.0,
            refresh_hz: 60.0,
        },
        3,
        None,
    )
    .expect("bind");
    let handle = server.handle();
    let accept = tokio::spawn(server.run_accept());

    assert_eq!(handle.client_count().await, 0);
    let client = UnixStream::connect(&socket).await.expect("connect");
    while handle.client_count().await == 0 {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert_eq!(handle.client_count().await, 1);

    drop(client);
    // Give the session task time to observe EOF and deregister.
    let mut waited = 0;
    while handle.client_count().await != 0 && waited < 200 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        waited += 1;
    }
    assert_eq!(handle.client_count().await, 0);

    accept.abort();
    let _ = std::fs::remove_file(&socket);
}
