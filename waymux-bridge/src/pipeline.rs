// SPDX-License-Identifier: Apache-2.0
//! Frame fanout pipeline: distributes encoded frames to all connected sessions.

use std::collections::HashMap;
use bytes::Bytes;
use tokio::sync::mpsc;
use crate::server::ClientSession;

/// Commands sent from the session acceptor to the pipeline to add or remove sessions.
pub enum PipelineCommand {
    /// Register a new client session for frame delivery.
    AddSession(ClientSession),
    /// Remove a session by ID (on disconnect).
    RemoveSession(u64),
}

/// The frame fanout pipeline.
///
/// Receives encoded [`Bytes`] frames from the calloop screencopy thread and
/// delivers them to all registered [`ClientSession`]s.
pub struct Pipeline {
    /// Active sessions indexed by session ID.
    sessions: HashMap<u64, ClientSession>,
    /// Receiver for encoded frames from the calloop thread.
    frame_rx: mpsc::Receiver<Bytes>,
    /// Receiver for add/remove session commands.
    command_rx: mpsc::Receiver<PipelineCommand>,
}

impl Pipeline {
    /// Create a new pipeline, returning it alongside the senders.
    ///
    /// - `frame_tx`: hand to the calloop thread to deliver encoded frames.
    /// - `command_tx`: hand to the session acceptor to add/remove sessions.
    pub fn new(
        frame_rx: mpsc::Receiver<Bytes>,
        command_rx: mpsc::Receiver<PipelineCommand>,
    ) -> Self {
        Pipeline {
            sessions: HashMap::new(),
            frame_rx,
            command_rx,
        }
    }

    /// Create a pipeline together with its channel pair.
    ///
    /// Returns `(pipeline, frame_tx, command_tx)`.
    pub fn new_pair(
        frame_rx: mpsc::Receiver<Bytes>,
    ) -> (Self, mpsc::Sender<PipelineCommand>) {
        let (command_tx, command_rx) = mpsc::channel(64);
        let pipeline = Pipeline::new(frame_rx, command_rx);
        (pipeline, command_tx)
    }

    /// Register a session to receive frames.
    pub fn add_session(&mut self, session: ClientSession) {
        let id = session.id();
        tracing::info!(session_id = id, "pipeline: session added");
        self.sessions.insert(id, session);
    }

    /// Remove a session by ID.
    pub fn remove_session(&mut self, id: u64) {
        if self.sessions.remove(&id).is_some() {
            tracing::info!(session_id = id, "pipeline: session removed");
        }
    }

    /// Run the pipeline until all senders are dropped.
    ///
    /// Fans out each arriving frame to every registered session.
    /// The `// PERF:` annotation below explains the clone strategy.
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                frame_opt = self.frame_rx.recv() => {
                    let frame = match frame_opt {
                        Some(f) => f,
                        None => break,
                    };
                    let dead_ids: Vec<u64> = Vec::new();
                    for (id, session) in &self.sessions {
                        // PERF: Bytes::clone is arc-clone, O(1) — no pixel data copied.
                        session.try_send_frame(frame.clone());
                        // Sessions that are disconnected will log a debug trace inside
                        // try_send_frame; they are cleaned up via RemoveSession commands
                        // from the reader task, so we don't collect dead IDs here.
                        let _ = id;
                    }
                    // Drop any sessions that have been explicitly marked dead.
                    for id in dead_ids {
                        self.sessions.remove(&id);
                    }
                }

                cmd_opt = self.command_rx.recv() => {
                    match cmd_opt {
                        Some(PipelineCommand::AddSession(s)) => self.add_session(s),
                        Some(PipelineCommand::RemoveSession(id)) => self.remove_session(id),
                        None => break,
                    }
                }
            }
        }
    }
}
