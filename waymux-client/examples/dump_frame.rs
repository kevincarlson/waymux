//! Connects to a running `waymux-bridge` socket and prints the first decoded
//! frame's geometry and display info, then exits. Handy for validating a
//! bridge without the full Android client.
//!
//! Usage: `cargo run --example dump_frame -- <socket-path>`

use std::path::Path;
use std::time::Duration;

use waymux_client::ClientState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket = std::env::args()
        .nth(1)
        .ok_or("usage: dump_frame <socket-path>")?;

    let state = ClientState::connect(Path::new(&socket)).await?;

    for _ in 0..500 {
        if let Some(frame) = state.latest_frame() {
            let head = &frame.pixels[..4.min(frame.pixels.len())];
            println!(
                "decoded frame {}x{} ({} bytes); first pixel BGRA = {head:?}",
                frame.width,
                frame.height,
                frame.pixels.len(),
            );
            if let Some(info) = state.display_info() {
                println!(
                    "display {}x{} scale {} @ {} Hz",
                    info.width, info.height, info.scale_factor, info.refresh_hz,
                );
            }
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    Err("no frame received within timeout".into())
}
