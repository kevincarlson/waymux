//! Wires a [`FrameSource`] to the encoder and fans encoded frames out to all
//! connected clients via a [`ServerHandle`].

use bytes::BytesMut;
use tokio::sync::mpsc;
use waymux_proto::{FrameFullMsg, WfpMessage, encode_wfp};

use crate::encoder::FrameEncoder;
use crate::error::BridgeError;
use crate::server::ServerHandle;
use crate::source::FrameSource;

/// Runs the capture → encode → broadcast loop until the source completes.
///
/// The source runs on its own task and feeds frames through a bounded channel
/// (`channel_depth`); the pipeline encodes each frame, frames it as a WFP
/// `FrameFull` message, and broadcasts it to every connected client.
///
/// # Errors
/// Returns an error if encoding, framing, or the source task fails.
pub async fn run<S>(
    source: S,
    mut encoder: Box<dyn FrameEncoder>,
    handle: ServerHandle,
    channel_depth: usize,
) -> Result<(), BridgeError>
where
    S: FrameSource + Send + 'static,
{
    let (tx, mut rx) = mpsc::channel(channel_depth.max(1));
    let source_task = tokio::spawn(source.run(tx));

    while let Some(frame) = rx.recv().await {
        let encoding = encoder.encoding();
        let data = encoder.encode(&frame)?;
        let msg = WfpMessage::FrameFull(FrameFullMsg {
            width: frame.width,
            height: frame.height,
            encoding,
            data,
        });
        let mut buf = BytesMut::new();
        encode_wfp(&msg, &mut buf)?;
        handle.broadcast(buf.freeze()).await;
    }

    // The channel closed: surface any error the source returned.
    match source_task.await {
        Ok(result) => result,
        Err(join_err) => Err(BridgeError::Source(join_err.to_string())),
    }
}
