//! `waymux-bridge` captures compositor frames, encodes them, and streams them
//! to Waymux clients over a Unix domain socket, while accepting input events
//! back from clients.
//!
//! This crate is structured around a [`source::FrameSource`] seam. The current
//! source is an animated test pattern ([`source::TestPatternSource`]) that lets
//! the full pipeline run without a Wayland compositor; a `wlr-screencopy`
//! capture backend implements the same trait in a later change.
//!
//! The daemon wiring lives in [`run`]: it binds the [`server::Server`], starts
//! the accept loop, and drives the [`pipeline`].

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod config;
pub mod encoder;
pub mod error;
pub mod pipeline;
pub mod server;
pub mod source;

pub use config::Config;
pub use error::BridgeError;

use config::SourceKind;
use source::{FrameSource, TestPatternSource, WaylandScreencopySource};
use tokio::sync::mpsc;
use waymux_proto::WipMessage;

/// Builds the daemon from `config` and runs it until the source completes.
///
/// # Errors
/// Returns a [`BridgeError`] if the source cannot start, the socket cannot be
/// bound, or the pipeline fails.
pub async fn run(config: Config) -> Result<(), BridgeError> {
    match config.source {
        SourceKind::TestPattern => {
            let source = TestPatternSource::new(config.width, config.height, config.max_fps);
            serve(source, &config, None).await
        }
        SourceKind::Wayland => {
            let source = WaylandScreencopySource::new(config.overlay_cursor, config.max_fps)?;
            // Bridge the server's tokio input channel to the capture thread's
            // calloop channel.
            let calloop_tx = source.input_sender();
            let (input_tx, mut input_rx) = mpsc::channel::<WipMessage>(256);
            tokio::spawn(async move {
                while let Some(msg) = input_rx.recv().await {
                    let _ = calloop_tx.send(msg);
                }
            });
            serve(source, &config, Some(input_tx)).await
        }
    }
}

/// Common wiring: bind the server, start accepting, and drive the pipeline.
async fn serve<S: FrameSource + Send + 'static>(
    source: S,
    config: &Config,
    input: Option<mpsc::Sender<WipMessage>>,
) -> Result<(), BridgeError> {
    let display_info = source.display_info();
    let server = server::Server::bind(
        &config.socket_path(),
        display_info,
        config.queue_capacity(),
        input,
    )?;
    let handle = server.handle();
    let accept = tokio::spawn(server.run_accept());

    let encoder = config.build_encoder();
    let result = pipeline::run(source, encoder, handle, config.channel_depth()).await;

    accept.abort();
    result
}
