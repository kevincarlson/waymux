// SPDX-License-Identifier: Apache-2.0

//! Waymux Bridge daemon entry point.
//!
//! All business logic lives in the library crate (`lib.rs`). This file only
//! parses the CLI, initialises logging and the tokio runtime, and calls
//! [`run`].

use color_eyre::eyre;
use clap::Parser;
use tracing_subscriber::EnvFilter;

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

    let (frame_tx, frame_rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(8);
    let enc = encoder::from_config(&config);
    let (pl, command_tx) = pipeline::Pipeline::new_pair(frame_rx);
    let mut socket_server = server::UnixSocketServer::bind(&config.socket_path)?;

    tracing::info!(socket = %config.socket_path.display(), "bridge ready");

    tokio::select! {
        _ = pl.run() => {
            tracing::info!("pipeline exited");
        }
        _ = accept_loop(&mut socket_server, command_tx) => {
            tracing::info!("accept loop exited");
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("received ctrl-c, shutting down");
        }
    }

    drop((frame_tx, enc));
    Ok(())
}

/// Accept connections and register them with the pipeline.
async fn accept_loop(
    server: &mut server::UnixSocketServer,
    command_tx: tokio::sync::mpsc::Sender<pipeline::PipelineCommand>,
) {
    loop {
        match server.accept().await {
            Ok((session, _frame_rx, _read_half)) => {
                let _ = command_tx
                    .send(pipeline::PipelineCommand::AddSession(session))
                    .await;
            }
            Err(e) => {
                tracing::error!("accept error: {e}");
                break;
            }
        }
    }
}
