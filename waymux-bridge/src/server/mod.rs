//! Unix socket server: accept loop, per-client session registry, and the
//! frame fan-out handle used by the pipeline.

mod session;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use bytes::Bytes;
use tokio::net::UnixListener;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tracing::{info, warn};
use waymux_proto::{DisplayInfoMsg, WfpMessage, WipMessage};

use crate::error::BridgeError;

pub use session::FrameQueue;

type Registry = Arc<Mutex<HashMap<u64, Arc<FrameQueue>>>>;

/// A bound Unix socket server ready to accept Waymux clients.
pub struct Server {
    listener: UnixListener,
    socket_path: PathBuf,
    registry: Registry,
    display_info: WfpMessage,
    queue_capacity: usize,
    input: Option<mpsc::Sender<WipMessage>>,
    next_id: AtomicU64,
}

impl Server {
    /// Binds the server to `socket_path`, removing any stale socket file.
    ///
    /// `input`, when present, receives the WIP events decoded from clients (for
    /// injection by the Wayland source); `None` drops inbound input.
    ///
    /// # Errors
    /// Returns an error if the socket cannot be removed or bound.
    pub fn bind(
        socket_path: &Path,
        display_info: DisplayInfoMsg,
        queue_capacity: usize,
        input: Option<mpsc::Sender<WipMessage>>,
    ) -> Result<Self, BridgeError> {
        match std::fs::remove_file(socket_path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(BridgeError::Io(err)),
        }
        let listener = UnixListener::bind(socket_path)?;
        info!(path = %socket_path.display(), "listening for clients");
        Ok(Self {
            listener,
            socket_path: socket_path.to_path_buf(),
            registry: Registry::default(),
            display_info: WfpMessage::DisplayInfo(display_info),
            queue_capacity,
            input,
            next_id: AtomicU64::new(0),
        })
    }

    /// Returns a cloneable handle for broadcasting frames to all clients.
    #[must_use]
    pub fn handle(&self) -> ServerHandle {
        ServerHandle {
            registry: self.registry.clone(),
        }
    }

    /// Accepts connections forever, spawning a session task per client.
    ///
    /// # Errors
    /// Returns an error if the listener fails irrecoverably.
    pub async fn run_accept(self) -> Result<(), BridgeError> {
        loop {
            let (stream, _addr) = self.listener.accept().await?;
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let queue = FrameQueue::new(self.queue_capacity);
            self.registry.lock().await.insert(id, queue.clone());
            info!(client = id, "client connected");

            let registry = self.registry.clone();
            let display_info = self.display_info.clone();
            let input = self.input.clone();
            tokio::spawn(async move {
                if let Err(err) = session::run(stream, queue, display_info, input).await {
                    warn!(client = id, error = %err, "session ended with error");
                }
                registry.lock().await.remove(&id);
                info!(client = id, "client disconnected");
            });
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Best-effort cleanup of the socket file on shutdown.
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// A cloneable handle the pipeline uses to fan frames out to every client.
#[derive(Clone)]
pub struct ServerHandle {
    registry: Registry,
}

impl ServerHandle {
    /// Pushes one framed WFP message to every connected client's queue.
    pub async fn broadcast(&self, frame: Bytes) {
        let registry = self.registry.lock().await;
        for queue in registry.values() {
            // PERF: Bytes::clone is an Arc refcount bump, not a buffer copy.
            queue.push(frame.clone()).await;
        }
    }

    /// Number of currently connected clients (used in tests).
    pub async fn client_count(&self) -> usize {
        self.registry.lock().await.len()
    }
}
