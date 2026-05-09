// SPDX-License-Identifier: Apache-2.0
//! Runtime configuration for waymux-bridge.

use std::path::PathBuf;
use clap::Parser;

/// Frame encoding choice exposed via the CLI and environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum EncodingChoice {
    /// Uncompressed BGRA8 passthrough — useful for debugging and benchmarking.
    Raw,
    /// Zstd-compressed BGRA8 — the recommended default for production use.
    Zstd,
}

/// Returns the default Unix socket path: `$TMPDIR/waymux.sock` or `/tmp/waymux.sock`.
pub fn default_socket_path() -> PathBuf {
    let dir = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(dir).join("waymux.sock")
}

/// Runtime configuration for the Waymux Bridge daemon.
///
/// All fields can be set via environment variable or CLI flag.
/// CLI flags take precedence over environment variables.
#[derive(Debug, Clone, Parser)]
#[clap(name = "waymux-bridge", about = "Waymux Bridge daemon")]
pub struct Config {
    /// Wayland display socket name (env: WAYLAND_DISPLAY).
    #[clap(long = "wayland-display", env = "WAYLAND_DISPLAY", default_value = "wayland-0")]
    pub wayland_display: String,

    /// Unix socket path to listen on (env: WAYMUX_SOCKET).
    #[clap(long = "socket", env = "WAYMUX_SOCKET", default_value_os_t = default_socket_path())]
    pub socket_path: PathBuf,

    /// Frame encoding format (env: WAYMUX_ENCODING).
    #[clap(long = "encoding", env = "WAYMUX_ENCODING", default_value = "zstd")]
    pub encoding: EncodingChoice,

    /// Zstd compression level 1–22 (env: WAYMUX_ZSTD_LEVEL).
    #[clap(long = "zstd-level", env = "WAYMUX_ZSTD_LEVEL", default_value_t = 3)]
    pub zstd_level: i32,

    /// Maximum frame rate cap in Hz (env: WAYMUX_MAX_FPS).
    #[clap(long = "max-fps", env = "WAYMUX_MAX_FPS", default_value_t = 60)]
    pub max_fps: u32,

    /// Tracing log filter (env: WAYMUX_LOG).
    #[clap(long = "log", env = "WAYMUX_LOG", default_value = "info")]
    pub log_filter: String,
}
