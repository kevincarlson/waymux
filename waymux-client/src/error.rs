//! Error type for the Waymux client core.

use std::io;

use thiserror::Error;
use waymux_proto::FrameEncoding;

/// Errors produced by the client's transport, decoding, and input paths.
#[derive(Debug, Error)]
pub enum ClientError {
    /// An I/O error from the Unix socket transport.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// A WFP/WIP message failed to encode or decode.
    #[error("protocol codec error: {0}")]
    Codec(#[from] waymux_proto::CodecError),

    /// Zstd decompression of a frame payload failed.
    #[error("zstd decompression failed: {0}")]
    Decompress(String),

    /// The bridge sent a frame in an encoding the client cannot decode.
    #[error("unsupported frame encoding: {0:?}")]
    UnsupportedEncoding(FrameEncoding),

    /// A decoded frame's size did not match its declared geometry.
    #[error("decoded frame size mismatch: expected {expected} bytes, got {actual}")]
    FrameSize {
        /// Expected `width * height * 4` bytes.
        expected: usize,
        /// Bytes actually produced by the decoder.
        actual: usize,
    },

    /// The connection to the bridge was closed.
    #[error("connection closed")]
    Closed,
}
