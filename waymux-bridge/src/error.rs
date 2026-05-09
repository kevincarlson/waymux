// SPDX-License-Identifier: Apache-2.0
//! [`BridgeError`] and [`EncoderError`] types for waymux-bridge.

use thiserror::Error;

/// Top-level error type for the Waymux Bridge.
#[derive(Debug, Error)]
pub enum BridgeError {
    /// The Wayland compositor connection could not be established.
    #[error("Wayland connection failed: {0}")]
    WaylandConnect(String),
    /// A required Wayland protocol global was not advertised by the compositor.
    #[error("Required Wayland global '{0}' not advertised by compositor")]
    MissingGlobal(&'static str),
    /// A screencopy frame capture operation failed.
    #[error("Screencopy frame capture failed: {0}")]
    ScreencopyFailed(String),
    /// The Unix domain socket server encountered an I/O error.
    #[error("Socket server error: {0}")]
    SocketServer(#[from] std::io::Error),
    /// The frame encoder produced an error.
    #[error("Frame encoding error: {0}")]
    Encoding(#[from] EncoderError),
    /// A codec protocol error occurred.
    #[error("Protocol error: {0}")]
    Protocol(#[from] waymux_proto::CodecError),
    /// The calloop event loop encountered an error.
    #[error("Event loop error: {0}")]
    EventLoop(String),
}

impl From<wayland_client::ConnectError> for BridgeError {
    fn from(e: wayland_client::ConnectError) -> Self {
        BridgeError::WaylandConnect(e.to_string())
    }
}

/// Errors produced by the frame encoder subsystem.
#[derive(Debug, Error)]
pub enum EncoderError {
    /// Zstd compression failed.
    #[error("Zstd compression failed: {0}")]
    ZstdCompress(String),
    /// An encoding format was requested that this build does not support.
    #[error("Unsupported encoding requested: {0:?}")]
    UnsupportedEncoding(waymux_proto::FrameEncoding),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_error_display_non_empty() {
        let errors: Vec<Box<dyn std::fmt::Display>> = vec![
            Box::new(BridgeError::WaylandConnect("test".into())),
            Box::new(BridgeError::MissingGlobal("test_global")),
            Box::new(BridgeError::ScreencopyFailed("test".into())),
            Box::new(BridgeError::EventLoop("test".into())),
        ];
        for e in errors {
            let s = e.to_string();
            assert!(!s.is_empty());
            assert!(!s.contains("unwrap"));
        }
    }

    #[test]
    fn encoder_error_display() {
        let e = EncoderError::ZstdCompress("fail".into());
        assert!(!e.to_string().is_empty());
    }
}
