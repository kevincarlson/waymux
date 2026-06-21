//! Error types for the bridge daemon.

use std::io;

use thiserror::Error;

/// Top-level error type returned by the bridge's fallible operations.
#[derive(Debug, Error)]
pub enum BridgeError {
    /// An I/O error from the Unix socket server or runtime.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// A frame failed to encode.
    #[error("frame encoding failed: {0}")]
    Encode(#[from] EncoderError),

    /// A WFP message failed to serialise.
    #[error("protocol codec error: {0}")]
    Codec(#[from] waymux_proto::CodecError),

    /// The frame source terminated abnormally.
    #[error("frame source error: {0}")]
    Source(String),
}

impl BridgeError {
    /// Process exit code to report for this error.
    ///
    /// Mirrors the codes documented in the package SPEC so operators and
    /// supervisors can distinguish failure classes.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            BridgeError::Io(_) => 1,
            BridgeError::Encode(_) | BridgeError::Codec(_) => 1,
            BridgeError::Source(_) => 3,
        }
    }
}

/// Errors produced by the frame encoder pipeline.
#[derive(Debug, Error)]
pub enum EncoderError {
    /// The frame's pixel buffer was smaller than its declared geometry.
    #[error(
        "frame buffer too small: need {expected} bytes for {width}x{height} stride {stride}, got {actual}"
    )]
    ShortBuffer {
        /// Expected `stride * height` bytes.
        expected: usize,
        /// Bytes actually present in the buffer.
        actual: usize,
        /// Frame width in pixels.
        width: u32,
        /// Frame height in pixels.
        height: u32,
        /// Row stride in bytes.
        stride: u32,
    },

    /// Zstd compression of a packed frame failed.
    #[error("zstd compression failed: {0}")]
    Zstd(io::Error),
}
