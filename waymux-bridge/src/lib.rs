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

use source::{FrameSource, TestPatternSource};

/// Builds the daemon from `config` and runs it until the source completes.
///
/// # Errors
/// Returns a [`BridgeError`] if the socket cannot be bound or the pipeline
/// fails.
pub async fn run(config: Config) -> Result<(), BridgeError> {
    let source = TestPatternSource::new(config.width, config.height, config.max_fps);
    let display_info = source.display_info();

    let server =
        server::Server::bind(&config.socket_path(), display_info, config.queue_capacity())?;
    let handle = server.handle();
    let accept = tokio::spawn(server.run_accept());

    let encoder = config.build_encoder();
    let result = pipeline::run(source, encoder, handle, config.channel_depth()).await;

    accept.abort();
    result
}
