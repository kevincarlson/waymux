// SPDX-License-Identifier: Apache-2.0

//! Waymux Bridge daemon entry point.
//!
//! All business logic lives in the library crate (`lib.rs`). This file only
//! parses the CLI, initialises logging and the tokio runtime, and calls
//! [`run`].

use std::sync::mpsc as std_mpsc;

use bytes::BytesMut;
use clap::Parser;
use color_eyre::eyre;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;
use waymux_proto::{codec::decode_wip, WipMessage};

use waymux_bridge::compositor;
use waymux_bridge::config::Config;
use waymux_bridge::error::BridgeError;
use waymux_bridge::{encoder, pipeline, server};

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let config = Config::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&config.log_filter))
        .init();

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run(config))?;
    Ok(())
}

/// Main async entry point for the bridge daemon.
async fn run(config: Config) -> Result<(), BridgeError> {
    tracing::info!("waymux-bridge starting");

    let enc = encoder::from_config(&config);
    let (frame_tx, frame_rx) = mpsc::channel::<bytes::Bytes>(8);
    let (pl, command_tx) = pipeline::Pipeline::new_pair(frame_rx);

    // Channel for WIP messages: tokio reader tasks → Wayland thread.
    // Unbounded std channel so that send() never blocks in async context.
    let (wip_tx, wip_rx) = std_mpsc::channel::<WipMessage>();

    // Connect to Wayland and spawn the event loop thread.
    let (wayland_client, event_queue) = compositor::connect(&config.wayland_display)?;
    let bridge_state = compositor::BridgeState::new(frame_tx.clone(), enc);

    let wayland_thread = std::thread::Builder::new()
        .name("wayland".into())
        .spawn(move || compositor::run_event_loop(wayland_client, event_queue, bridge_state, wip_rx))
        .map_err(BridgeError::SocketServer)?;

    let mut socket_server = server::UnixSocketServer::bind(&config.socket_path)?;

    tracing::info!(socket = %config.socket_path.display(), "bridge ready, awaiting clients");

    tokio::select! {
        _ = pl.run() => {
            tracing::info!("pipeline exited");
        }
        _ = accept_loop(&mut socket_server, command_tx, wip_tx) => {
            tracing::info!("accept loop exited");
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("received ctrl-c, shutting down");
        }
    }

    drop(frame_tx);
    let _ = wayland_thread.join();
    Ok(())
}

/// Accept client connections and spawn per-client writer + reader tasks.
async fn accept_loop(
    server: &mut server::UnixSocketServer,
    command_tx: mpsc::Sender<pipeline::PipelineCommand>,
    wip_tx: std_mpsc::Sender<WipMessage>,
) {
    loop {
        match server.accept().await {
            Ok((session, frame_rx, read_half, write_half)) => {
                let id = session.id();

                // Register with the pipeline so this session receives frames.
                if command_tx
                    .send(pipeline::PipelineCommand::AddSession(session))
                    .await
                    .is_err()
                {
                    tracing::warn!("pipeline gone; stopping accept loop");
                    break;
                }

                // Deliver frames to the client over the write half.
                server::spawn_writer(frame_rx, write_half);

                // Read WIP messages from the client and forward to the Wayland thread.
                let wip_tx2 = wip_tx.clone();
                let cmd_tx2 = command_tx.clone();
                tokio::spawn(async move {
                    wip_reader_loop(id, read_half, wip_tx2, cmd_tx2).await;
                });
            }
            Err(e) => {
                tracing::error!("accept error: {e}");
                break;
            }
        }
    }
}

/// Read length-prefixed WIP messages from a client socket until EOF or error.
///
/// Decoded messages are forwarded to the Wayland thread via `wip_tx`.
/// On disconnect, a `RemoveSession` command is sent to the pipeline.
async fn wip_reader_loop(
    session_id: u64,
    mut read_half: tokio::net::unix::OwnedReadHalf,
    wip_tx: std_mpsc::Sender<WipMessage>,
    command_tx: mpsc::Sender<pipeline::PipelineCommand>,
) {
    use tokio::io::AsyncReadExt as _;

    let mut buf = BytesMut::with_capacity(512);

    loop {
        match read_half.read_buf(&mut buf).await {
            Ok(0) => break, // EOF — client closed connection
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(session_id, "WIP read error: {e}");
                break;
            }
        }

        // Drain all complete WIP frames from the accumulation buffer.
        loop {
            match decode_wip(&mut buf) {
                Ok(Some(msg)) => {
                    if wip_tx.send(msg).is_err() {
                        tracing::warn!(session_id, "WIP channel closed; stopping reader");
                        let _ = command_tx
                            .send(pipeline::PipelineCommand::RemoveSession(session_id))
                            .await;
                        return;
                    }
                }
                Ok(None) => break, // need more bytes
                Err(e) => {
                    tracing::warn!(session_id, "WIP decode error: {e}");
                    break;
                }
            }
        }
    }

    tracing::info!(session_id, "client disconnected");
    let _ = command_tx
        .send(pipeline::PipelineCommand::RemoveSession(session_id))
        .await;
}
