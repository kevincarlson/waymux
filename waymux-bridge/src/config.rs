//! Daemon configuration parsed from environment variables and CLI flags.

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::encoder::{FrameEncoder, RawBgra8Encoder, ZstdBgra8Encoder};

/// Frame encoding selectable by the operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Encoding {
    /// Uncompressed BGRA8 passthrough.
    Raw,
    /// Zstd-compressed BGRA8 (default).
    Zstd,
}

/// Where the bridge sources frames from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SourceKind {
    /// Capture a real Wayland compositor via `wlr-screencopy`.
    Wayland,
    /// Emit a built-in animated test pattern (no compositor required).
    TestPattern,
}

/// Bridge daemon configuration.
///
/// Every field has an environment-variable source and a CLI override, matching
/// the table in `waymux-bridge/docs/spec.md`. The `width`/`height` fields apply
/// only to the test-pattern source; the Wayland source takes its geometry from
/// the captured output.
#[derive(Debug, Clone, Parser)]
#[command(name = "waymux-bridge", version, about)]
pub struct Config {
    /// Unix socket path to listen on (default: `$TMPDIR/waymux.sock`).
    #[arg(long, env = "WAYMUX_SOCKET")]
    socket: Option<PathBuf>,

    /// Frame source: `wayland` (capture a compositor) or `test-pattern`.
    #[arg(long, env = "WAYMUX_SOURCE", default_value = "wayland")]
    pub source: SourceKind,

    /// Draw the cursor into captured frames (Wayland source only).
    #[arg(long, env = "WAYMUX_OVERLAY_CURSOR", default_value_t = true)]
    pub overlay_cursor: bool,

    /// Frame encoding to apply before transmission.
    #[arg(long, env = "WAYMUX_ENCODING", default_value = "zstd")]
    pub encoding: Encoding,

    /// Zstd compression level (1–22).
    #[arg(long, env = "WAYMUX_ZSTD_LEVEL", default_value_t = 3)]
    pub zstd_level: i32,

    /// Maximum frame rate cap.
    #[arg(long, env = "WAYMUX_MAX_FPS", default_value_t = 60)]
    pub max_fps: u32,

    /// Test-pattern source width in pixels.
    #[arg(long, env = "WAYMUX_WIDTH", default_value_t = 1280)]
    pub width: u32,

    /// Test-pattern source height in pixels.
    #[arg(long, env = "WAYMUX_HEIGHT", default_value_t = 720)]
    pub height: u32,
}

impl Config {
    /// Resolves the socket path, defaulting to `$TMPDIR/waymux.sock`.
    #[must_use]
    pub fn socket_path(&self) -> PathBuf {
        if let Some(path) = &self.socket {
            return path.clone();
        }
        let base = std::env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
        base.join("waymux.sock")
    }

    /// Builds the configured frame encoder.
    #[must_use]
    pub fn build_encoder(&self) -> Box<dyn FrameEncoder> {
        match self.encoding {
            Encoding::Raw => Box::new(RawBgra8Encoder::new()),
            Encoding::Zstd => Box::new(ZstdBgra8Encoder::new(self.zstd_level)),
        }
    }

    /// Maximum number of frames queued per client before the oldest is dropped.
    #[must_use]
    pub fn queue_capacity(&self) -> usize {
        3
    }

    /// Depth of the source → pipeline hand-off channel.
    #[must_use]
    pub fn channel_depth(&self) -> usize {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Config {
        Config::try_parse_from(args).expect("args should parse")
    }

    #[test]
    fn defaults_are_applied() {
        let cfg = parse(&["waymux-bridge"]);
        assert_eq!(cfg.encoding, Encoding::Zstd);
        assert_eq!(cfg.zstd_level, 3);
        assert_eq!(cfg.max_fps, 60);
        assert_eq!(cfg.width, 1280);
        assert_eq!(cfg.height, 720);
        assert!(cfg.socket_path().ends_with("waymux.sock"));
    }

    #[test]
    fn cli_overrides_are_parsed() {
        let cfg = parse(&[
            "waymux-bridge",
            "--encoding",
            "raw",
            "--max-fps",
            "30",
            "--width",
            "800",
            "--height",
            "600",
            "--socket",
            "/run/custom.sock",
        ]);
        assert_eq!(cfg.encoding, Encoding::Raw);
        assert_eq!(cfg.max_fps, 30);
        assert_eq!(cfg.width, 800);
        assert_eq!(cfg.height, 600);
        assert_eq!(cfg.socket_path(), PathBuf::from("/run/custom.sock"));
    }

    #[test]
    fn invalid_encoding_is_rejected() {
        assert!(Config::try_parse_from(["waymux-bridge", "--encoding", "h264"]).is_err());
    }
}
