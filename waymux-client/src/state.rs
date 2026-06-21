//! [`ClientState`] owns the background connection task and exposes the latest
//! decoded frame plus an input channel.
//!
//! The render/JNI thread reads [`ClientState::latest_frame`] synchronously, so
//! the shared snapshots use `std::sync::Mutex`. The lock is only ever held for
//! a non-blocking get/set and never across an `.await`, so it cannot stall the
//! async runtime.

use std::path::Path;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{trace, warn};
use waymux_proto::{DisplayInfoMsg, WfpMessage, WipMessage};

use crate::connection::{self, Reader, Writer};
use crate::decoder::{DecodedFrame, decode_full};
use crate::error::ClientError;

/// Snapshots shared between the connection task and the render thread.
#[derive(Default)]
struct Shared {
    latest: Mutex<Option<DecodedFrame>>,
    display: Mutex<Option<DisplayInfoMsg>>,
}

/// A live client session: a background task pumping frames and input.
pub struct ClientState {
    shared: Arc<Shared>,
    outbound: mpsc::Sender<WipMessage>,
    task: JoinHandle<()>,
}

impl ClientState {
    /// Connects to the bridge at `path` and starts the connection task.
    ///
    /// # Errors
    /// Returns [`ClientError::Io`] if the socket cannot be reached.
    pub async fn connect(path: &Path) -> Result<Self, ClientError> {
        let (reader, writer) = connection::connect(path).await?;
        let (outbound, inbound) = mpsc::channel(64);
        let shared = Arc::new(Shared::default());
        let task = tokio::spawn(run(
            reader,
            writer,
            inbound,
            shared.clone(),
            outbound.clone(),
        ));
        Ok(Self {
            shared,
            outbound,
            task,
        })
    }

    /// Returns the most recently decoded frame, if any.
    #[must_use]
    pub fn latest_frame(&self) -> Option<DecodedFrame> {
        self.shared
            .latest
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Returns the latest display geometry advertised by the bridge.
    #[must_use]
    pub fn display_info(&self) -> Option<DisplayInfoMsg> {
        self.shared.display.lock().ok().and_then(|guard| *guard)
    }

    /// Queues an input event for transmission to the bridge.
    ///
    /// # Errors
    /// Returns [`ClientError::Closed`] if the connection task has stopped.
    pub async fn send_input(&self, msg: WipMessage) -> Result<(), ClientError> {
        self.outbound
            .send(msg)
            .await
            .map_err(|_| ClientError::Closed)
    }

    /// Queues an input event without awaiting, for callers on a non-async
    /// thread (e.g. the Android UI thread).
    ///
    /// Under backpressure the event is dropped rather than blocking, since
    /// input is high-frequency and the freshest events matter most.
    ///
    /// # Errors
    /// Returns [`ClientError::Closed`] if the connection task has stopped.
    pub fn try_send_input(&self, msg: WipMessage) -> Result<(), ClientError> {
        use tokio::sync::mpsc::error::TrySendError;
        match self.outbound.try_send(msg) {
            Ok(()) | Err(TrySendError::Full(_)) => Ok(()),
            Err(TrySendError::Closed(_)) => Err(ClientError::Closed),
        }
    }
}

impl Drop for ClientState {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Connection task: multiplexes inbound frames and outbound input.
async fn run(
    mut reader: Reader,
    mut writer: Writer,
    mut inbound: mpsc::Receiver<WipMessage>,
    shared: Arc<Shared>,
    pong: mpsc::Sender<WipMessage>,
) {
    loop {
        tokio::select! {
            incoming = reader.next() => match incoming {
                Ok(Some(msg)) => {
                    if !handle(msg, &shared, &pong) {
                        break;
                    }
                }
                Ok(None) => break,
                Err(err) => {
                    warn!(error = %err, "connection read failed");
                    break;
                }
            },
            outgoing = inbound.recv() => match outgoing {
                Some(msg) => {
                    if let Err(err) = writer.send(&msg).await {
                        warn!(error = %err, "connection write failed");
                        break;
                    }
                }
                None => break,
            },
        }
    }
}

/// Applies one inbound message; returns `false` to end the session.
fn handle(msg: WfpMessage, shared: &Shared, pong: &mpsc::Sender<WipMessage>) -> bool {
    match msg {
        WfpMessage::DisplayInfo(info) => {
            if let Ok(mut guard) = shared.display.lock() {
                *guard = Some(info);
            }
        }
        WfpMessage::FrameFull(frame) => match decode_full(&frame) {
            Ok(decoded) => {
                if let Ok(mut guard) = shared.latest.lock() {
                    *guard = Some(decoded);
                }
            }
            Err(err) => warn!(error = %err, "frame decode failed"),
        },
        WfpMessage::FrameDamage(_) => trace!("damage frames are handled in M5"),
        WfpMessage::Ping { sequence } => {
            // Best-effort: drop the pong if the outbound queue is saturated.
            let _ = pong.try_send(WipMessage::Pong { sequence });
        }
        WfpMessage::Disconnect { reason } => {
            trace!(?reason, "bridge requested disconnect");
            return false;
        }
    }
    true
}
