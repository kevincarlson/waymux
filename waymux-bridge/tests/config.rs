// SPDX-License-Identifier: Apache-2.0
//! Integration tests for waymux-bridge Config parsing.

use serial_test::serial;
use clap::Parser as _;
use waymux_bridge::config::{Config, EncodingChoice, default_socket_path};

#[test]
#[serial]
fn test_defaults() {
    // Parse with no args — all defaults should apply.
    let cfg = Config::parse_from(["waymux-bridge"]);
    assert_eq!(cfg.wayland_display, "wayland-0");
    assert_eq!(cfg.encoding, EncodingChoice::Zstd);
    assert_eq!(cfg.zstd_level, 3);
    assert_eq!(cfg.max_fps, 60);
    assert_eq!(cfg.log_filter, "info");
}

#[test]
#[serial]
fn test_env_override() {
    // SAFETY: test is single-threaded via serial_test; no other thread reads
    // these env vars concurrently.
    unsafe {
        std::env::set_var("WAYMUX_ENCODING", "raw");
        std::env::set_var("WAYMUX_MAX_FPS", "30");
    }

    let cfg = Config::parse_from(["waymux-bridge"]);
    assert_eq!(cfg.encoding, EncodingChoice::Raw);
    assert_eq!(cfg.max_fps, 30);

    // SAFETY: same guarantee as above.
    unsafe {
        std::env::remove_var("WAYMUX_ENCODING");
        std::env::remove_var("WAYMUX_MAX_FPS");
    }
}

#[test]
#[serial]
fn test_cli_override() {
    // SAFETY: single-threaded via serial_test.
    unsafe {
        std::env::set_var("WAYMUX_ENCODING", "raw");
    }

    let cfg = Config::parse_from(["waymux-bridge", "--encoding", "zstd", "--zstd-level", "9"]);
    assert_eq!(cfg.encoding, EncodingChoice::Zstd);
    assert_eq!(cfg.zstd_level, 9);

    // SAFETY: single-threaded via serial_test.
    unsafe {
        std::env::remove_var("WAYMUX_ENCODING");
    }
}

#[test]
#[serial]
fn test_socket_default_contains_tmpdir() {
    // SAFETY: single-threaded via serial_test.
    unsafe {
        std::env::remove_var("TMPDIR");
        std::env::remove_var("WAYMUX_SOCKET");
    }

    let path = default_socket_path();
    // Default path must end with "waymux.sock".
    assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("waymux.sock"));
    // Default directory must be /tmp when TMPDIR is unset.
    assert!(path.to_str().map(|s| s.contains("tmp")).unwrap_or(false));
}
