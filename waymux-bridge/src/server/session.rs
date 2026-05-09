// SPDX-License-Identifier: Apache-2.0
//! Per-client session state and frame delivery.

use bytes::Bytes;
use tokio::net::unix::OwnedWriteHalf;
use tokio::sync::mpsc;
use tokio::io::AsyncWriteExt as _;

/// A connected Waymux Client session.
///
/// Holds the outbound frame queue sender. Frames are delivered via
/// [`ClientSession::try_send_frame`]; when the queue is full the
/// incoming frame is dropped to avoid latency build-up.
pub struct ClientSession {
    /// Unique session identifier assigned at accept time.
    pub id: u64,
    /// Sender for outbound encoded frames. Bounded to capacity 4.
    frame_tx: mpsc::Sender<Bytes>,
    /// Human-readable peer description for logging.
    peer: String,
}

impl ClientSession {
    /// Create a new session and the receiver half of the frame queue.
    ///
    /// Pass the returned [`mpsc::Receiver<Bytes>`] to [`spawn_writer`].
    pub fn new(id: u64, peer: String) -> (Self, mpsc::Receiver<Bytes>) {
        let (frame_tx, frame_rx) = mpsc::channel(4);
        (ClientSession { id, frame_tx, peer }, frame_rx)
    }

    /// Returns the session identifier.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Attempt to enqueue a frame for delivery to this client.
    ///
    /// If the queue is full the frame is silently dropped and a
    /// [`tracing::warn!`] is emitted. If the receiver is gone,
    /// a debug trace is emitted and the frame is discarded.
    pub fn try_send_frame(&self, frame: Bytes) {
        match self.frame_tx.try_send(frame) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(
                    session_id = self.id,
                    peer = %self.peer,
                    "frame dropped: session send queue full"
                );
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::debug!(session_id = self.id, "session disconnected");
            }
        }
    }
}

/// Spawn a writer task that reads frames from `rx` and writes them to `write_half`.
///
/// Each frame is written as a raw byte sequence (already length-prefixed by the
/// encoder pipeline). The task exits when the receiver is closed or when a
/// write error occurs.
pub fn spawn_writer(
    mut rx: mpsc::Receiver<Bytes>,
    mut write_half: OwnedWriteHalf,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(frame) = rx.recv().await {
            if write_half.write_all(&frame).await.is_err() {
                break;
            }
        }
    })
}
