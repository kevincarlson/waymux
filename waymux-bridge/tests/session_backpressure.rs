// SPDX-License-Identifier: Apache-2.0
//! Integration tests for ClientSession back-pressure behaviour.

use bytes::Bytes;
use waymux_bridge::server::session::ClientSession;

/// Helper: create a session with id=1 and a pre-set peer string.
fn make_session() -> (ClientSession, tokio::sync::mpsc::Receiver<Bytes>) {
    ClientSession::new(1, "test-peer".to_string())
}

#[tokio::test]
async fn frames_dropped_when_full() {
    let (session, mut rx) = make_session();

    // Send 6 frames — the channel capacity is 4, so 2 should be dropped.
    for i in 0u8..6 {
        session.try_send_frame(Bytes::from(vec![i]));
    }

    // Drain whatever was received — must be at most 4 frames.
    let mut received = 0usize;
    while rx.try_recv().is_ok() {
        received += 1;
    }
    assert!(
        received <= 4,
        "expected at most 4 frames in a capacity-4 channel, got {received}"
    );
}

#[tokio::test]
async fn disconnected_session_no_panic() {
    let (session, rx) = make_session();
    // Drop the receiver to simulate a disconnected client.
    drop(rx);

    // Sending to a closed channel must not panic.
    for _ in 0..3 {
        session.try_send_frame(Bytes::from_static(b"hello"));
    }
    // If we reached here, no panic occurred.
}
