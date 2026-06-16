//! `waymux-bridge` daemon entry point: parse config, init logging, run.

use std::process::ExitCode;

use clap::Parser;
use tracing_subscriber::EnvFilter;
use waymux_bridge::{Config, run};

fn main() -> ExitCode {
    let config = Config::parse();
    init_logging();

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("waymux-bridge: failed to start async runtime: {err}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = %err, "bridge exited with error");
            eprintln!("waymux-bridge: {err}");
            ExitCode::from(err.exit_code())
        }
    }
}

/// Initialises `tracing` using the `WAYMUX_LOG` filter (default `info`).
fn init_logging() {
    let filter = EnvFilter::try_from_env("WAYMUX_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}
