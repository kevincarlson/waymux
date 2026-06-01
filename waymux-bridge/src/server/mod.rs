// SPDX-License-Identifier: Apache-2.0
//! Unix domain socket server for accepting Waymux Client connections.

pub mod session;
pub use session::{ClientSession, spawn_writer};

use std::path::Path;
use bytes::Bytes;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use crate::error::BridgeError;

/// Listens on a Unix domain socket and accepts Waymux client connections.
pub struct UnixSocketServer {
    listener: UnixListener,
    /// Monotonically increasing session ID counter.
    next_id: u64,
}

impl UnixSocketServer {
    /// Bind to `socket_path`, removing any stale socket file first.
    ///
    /// Stale files are removed because [`UnixListener::bind`] fails if the
    /// path already exists, even when no process is listening on it.
    pub fn bind(socket_path: &Path) -> Result<Self, BridgeError> {
        match std::fs::remove_file(socket_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(BridgeError::SocketServer(e)),
        }
        let listener = UnixListener::bind(socket_path)?;
        tracing::info!(path = %socket_path.display(), "listening on Unix socket");
        Ok(UnixSocketServer { listener, next_id: 1 })
    }

    /// Accept one connection and split it into its constituent parts.
    ///
    /// Returns `(session, frame_rx, read_half, write_half)`.
    /// - Pass `session` to the pipeline via [`crate::pipeline::PipelineCommand::AddSession`].
    /// - Pass `frame_rx` and `write_half` to [`spawn_writer`] to deliver frames.
    /// - Spawn a reader task on `read_half` to process inbound WIP messages.
    pub async fn accept(
        &mut self,
    ) -> Result<(ClientSession, mpsc::Receiver<Bytes>, OwnedReadHalf, OwnedWriteHalf), BridgeError>
    {
        let (stream, addr) = self.listener.accept().await?;
        let peer = format!("{addr:?}");
        let id = self.next_id;
        self.next_id += 1;
        let (read_half, write_half) = stream.into_split();
        let (session, frame_rx) = ClientSession::new(id, peer.clone());
        tracing::info!(session_id = id, peer = %peer, "client connected");
        Ok((session, frame_rx, read_half, write_half))
    }
}
