# waymux-bridge — Package Specification

**Crate:** `waymux-bridge`  
**Type:** Binary (`bin`)  
**Version:** 0.1.0  
**License:** Apache-2.0  
**Target:** `aarch64-linux-android` (Termux), also buildable for `x86_64-unknown-linux-gnu` (development)

---

## Purpose

`waymux-bridge` is a daemon that runs inside Termux. It connects to a Wayland compositor as a client, captures frame output using the `wlr-screencopy-unstable-v1` or `ext-image-capture-source-v1` Wayland protocol extension, encodes frames, and streams them to connected Waymux Client instances over a Unix domain socket. It also receives Waymux Input Protocol (WIP) messages from clients and injects them into the compositor using `zwp-virtual-keyboard-v1` and `zwlr-virtual-pointer-v1`.

---

## Dependencies

```toml
[dependencies]
waymux-proto              = { path = "../waymux-proto" }
smithay-client-toolkit    = "0.19"
wayland-client            = "0.31"
wayland-protocols         = "0.31"
wayland-protocols-wlr     = "0.2"
tokio                     = { version = "1", features = ["full"] }
zstd                      = "0.13"
bytes                     = "1"
thiserror                 = "1"
tracing                   = "0.1"
tracing-subscriber        = { version = "0.3", features = ["env-filter"] }
clap                      = { version = "4", features = ["derive", "env"] }
nix                       = { version = "0.27", features = ["socket", "fs"] }
```

---

## Module Layout

```
waymux-bridge/src/
├── main.rs            # Entry: arg parsing, runtime, top-level error display
├── config.rs          # Config struct: socket path, encoding, port settings
├── error.rs           # BridgeError type hierarchy
├── compositor/
│   ├── mod.rs         # CompositorClient state machine
│   ├── screencopy.rs  # wlr-screencopy / ext-image-capture-source handler
│   └── input.rs       # Virtual pointer + keyboard injection
├── server/
│   ├── mod.rs         # UnixSocketServer: accept loop
│   └── session.rs     # ClientSession: per-client state, send queue
├── encoder/
│   ├── mod.rs         # FrameEncoder trait + dispatch
│   ├── raw.rs         # RawBgra8Encoder (passthrough)
│   └── zstd.rs        # ZstdBgra8Encoder
└── pipeline.rs        # Connects compositor output → encoder → session fanout
```

---

## Configuration

Configured via environment variables (with CLI flag overrides):

| Variable | CLI Flag | Default | Description |
|---|---|---|---|
| `WAYLAND_DISPLAY` | — | `wayland-0` | Wayland socket to connect to |
| `WAYMUX_SOCKET` | `--socket` | `$TMPDIR/waymux.sock` | Unix socket path to listen on |
| `WAYMUX_ENCODING` | `--encoding` | `zstd` | Frame encoding: `raw`, `zstd` |
| `WAYMUX_ZSTD_LEVEL` | `--zstd-level` | `3` | Zstd compression level (1–22) |
| `WAYMUX_MAX_FPS` | `--max-fps` | `60` | Maximum frame rate cap |
| `WAYMUX_LOG` | — | `info` | `tracing` log filter |

---

## Startup Sequence

```
1. Parse config (env + CLI)
2. Connect to Wayland compositor via $WAYLAND_DISPLAY
3. Enumerate globals; verify wlr-screencopy or ext-image-capture-source is present
   → If absent: exit with descriptive error code 2
4. Instantiate virtual-keyboard and virtual-pointer globals
5. Bind screencopy/capture-source to the primary output
6. Create encoder pipeline (based on WAYMUX_ENCODING)
7. Start Unix socket server on WAYMUX_SOCKET
8. Enter main event loop:
   a. Compositor events processed on calloop thread
   b. Socket I/O processed on tokio runtime
   c. Frame-ready events bridge calloop → tokio via tokio::sync::mpsc channel
```

---

## CompositorClient State Machine

States: `Connecting → GlobalsEnumerated → ScreencopyBound → Capturing → Error`

Transitions:
- `Connecting → GlobalsEnumerated`: all required Wayland globals found in registry.
- `GlobalsEnumerated → ScreencopyBound`: screencopy/capture-source object created for the primary output.
- `ScreencopyBound → Capturing`: first frame request sent; frame ready callback fires.
- Any state → `Error`: Wayland connection drops or required global disappears.

On reaching `Error`, the bridge attempts reconnection after a 2-second back-off, up to 5 retries before exiting with error code 3.

---

## Frame Capture Flow

1. Bridge sends a `zwlr_screencopy_frame_v1::copy` request (or equivalent for ext-image-capture-source) to the compositor for the primary output.
2. Compositor sends back buffer format/stride info via `buffer` event.
3. Bridge allocates a shared memory buffer (via `wl_shm`) matching the advertised format.
4. Compositor writes frame data to the shm buffer and signals `ready`.
5. Bridge reads pixel data from shm, passes it through the encoder pipeline.
6. Encoder produces a `bytes::Bytes` payload.
7. A `WfpMessage::FrameFull` (or `FrameDamage` if damage hints are present) is constructed and enqueued to all active `ClientSession` send queues.
8. Bridge immediately requests the next frame to sustain the configured FPS cap.

### Damage Region Optimization

If the compositor provides damage hints (via `damage` events in wlr-screencopy v2 or ext protocol), the bridge will:
- Accumulate damage rectangles for the current frame.
- Encode only the union of damaged regions.
- Send `WfpMessage::FrameDamage` instead of `FrameFull`.

When no damage hints are available, always send `FrameFull`.

---

## Input Injection

On receiving a `WipMessage` from a `ClientSession`:

| WIP Message | Wayland Injection |
|---|---|
| `PointerMotion` | `zwlr_virtual_pointer_v1::motion_absolute` |
| `PointerButton` | `zwlr_virtual_pointer_v1::button` |
| `PointerAxis` | `zwlr_virtual_pointer_v1::axis` |
| `TouchDown/Motion/Up` | `zwlr_virtual_pointer_v1::motion_absolute` + button emulation (v1 limitation) |
| `StylusDown/Motion/Up` | `zwlr_virtual_pointer_v1` with pressure metadata (if compositor supports) |
| `KeyDown/Up` | `zwp_virtual_keyboard_v1::key` |

Coordinate normalization: WIP coordinates are logical pixels in compositor space (sent by the client after denormalization). The bridge forwards them directly without further transformation.

---

## ClientSession

Each connected Android client has an associated `ClientSession`:

- Owns a `tokio::sync::mpsc::Sender<bytes::Bytes>` for outbound frames.
- Owns a reader task that processes incoming WIP messages from the socket.
- On connect: sends `WfpMessage::DisplayInfo` immediately.
- Handles backpressure: if the client's send queue is full (> 3 queued frames), the oldest frame is dropped (not the newest) to maintain low latency.
- On disconnect: session is removed from the active set; its send queue is dropped.

---

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Wayland connection failed: {0}")]
    WaylandConnect(#[from] wayland_client::ConnectError),

    #[error("Required Wayland global '{0}' not advertised by compositor")]
    MissingGlobal(&'static str),

    #[error("Screencopy frame capture failed: {0}")]
    ScreencopyFailed(String),

    #[error("Socket server error: {0}")]
    SocketServer(#[from] std::io::Error),

    #[error("Frame encoding error: {0}")]
    Encoding(#[from] EncoderError),

    #[error("Protocol error: {0}")]
    Protocol(#[from] waymux_proto::CodecError),
}
```

---

## Testing Requirements

- `tests/config.rs`: verify config parsing from env vars and CLI args, including defaults.
- `tests/encoder_raw.rs`: encode a synthetic BGRA8 frame and verify byte layout.
- `tests/encoder_zstd.rs`: encode with zstd, decode with `zstd::decode_all`, verify pixel equality.
- `tests/session_backpressure.rs`: simulate a slow client, verify old frames are dropped and new ones are delivered.
- `tests/protocol_injection.rs`: integration test using a headless Smithay compositor; send WIP messages and assert they produce the correct Wayland events.

For Wayland integration tests, use Smithay's `wayland-server` in a test harness with an in-process compositor mock.
