# AppThere Waymux — High-Level Specification

**Version:** 0.1.0-draft  
**Status:** Draft  
**Repository:** `appthere/waymux` (monorepo)  
**License:** Apache-2.0

---

## 1. Vision

AppThere Waymux enables a full Linux desktop experience on Android devices running Termux, by efficiently routing graphical output from a Wayland compositor in Termux to a native Android display layer and forwarding all input events from the Android layer back to the compositor. The project also includes **Light Speed Desktop**, a lightweight, responsive Wayland compositor and desktop environment built in Rust, designed to be the first-class compositor target for Waymux.

The three sub-projects are independently usable but designed to work together as a cohesive system.

---

## 2. Sub-Projects

| Sub-project | Crate/Package | Language | Role |
|---|---|---|---|
| Waymux Client | `waymux-client` | Kotlin + Rust (JNI) | Android app: display + input |
| Waymux Bridge | `waymux-bridge` | Rust | Termux daemon: IPC relay |
| Light Speed Desktop | `light-speed-desktop` | Rust | Wayland compositor + DE |

### 2.1 Waymux Client

An Android application that:
- Connects to the Waymux Bridge over a local Unix domain socket (via Termux shared storage path, using the `sharedUserId` mechanism or the `WAYMUX_SOCKET` environment convention).
- Receives a stream of encoded frame buffers (or damage regions) from the Bridge and renders them to a `SurfaceView` backed by a wgpu Vulkan surface via Rust JNI.
- Captures all touch, stylus (Android Pen API), multi-touch gesture, mouse (USB/Bluetooth HID), and hardware keyboard events and serializes them into the Waymux Input Protocol (WIP) for transmission back to the Bridge.
- Provides a minimal Kotlin Activity shell that manages the Android lifecycle and delegates all rendering and input handling to a Rust library (`libwaymux_client.so`).

### 2.2 Waymux Bridge

A Linux daemon (targeting Termux's aarch64-linux-android environment) that:
- Acts as a Wayland protocol client to an existing compositor (e.g., Light Speed Desktop or any other Wayland compositor) running in the same Termux session via the standard `$WAYLAND_DISPLAY` socket.
- Captures compositor output using the `wlr-screencopy` or `ext-image-capture-source` Wayland protocol extensions.
- Listens for Android client connections on a local Unix domain socket (`$TMPDIR/waymux.sock`).
- Streams encoded frames to connected clients.
- Deserializes WIP input events from clients and injects them into the compositor via the `wlr-virtual-pointer` and `zwp-virtual-keyboard` Wayland protocols.
- Handles connection lifecycle, reconnect, and backpressure.

### 2.3 Light Speed Desktop

A Wayland compositor and desktop environment built on Smithay that:
- Implements a hybrid stacking/tiling window manager.
- Uses responsive layout breakpoints to adapt its chrome and window arrangement to the available display size and orientation.
- Supports multiple output backends: DRM/KMS (bare-metal), Winit (nested/development), and the Waymux virtual output (for Bridge integration).
- Uses Iced (preferred) or GTK4 as the widget toolkit for its own UI chrome (panels, launchers, notifications).
- Provides XWayland support for legacy X11 applications.
- Exposes the `ext-image-capture-source` and `wlr-screencopy` protocol extensions required by the Waymux Bridge.

---

## 3. System Architecture

```
┌──────────────────────────────────────────────────────────┐
│                      Android Layer                       │
│                                                          │
│  ┌─────────────────────────────────────────────────┐    │
│  │           Waymux Client (Kotlin + Rust)          │    │
│  │   ┌────────────┐   ┌──────────────────────────┐ │    │
│  │   │  Activity  │   │  libwaymux_client.so      │ │    │
│  │   │  (Kotlin)  │──▶│  • Frame decoder (wgpu)   │ │    │
│  │   │            │   │  • Input serializer (WIP) │ │    │
│  │   └────────────┘   └──────────────────────────┘ │    │
│  └──────────────┬───────────────────▲───────────────┘    │
│                 │  Unix socket      │                     │
│           frames│  (shared fs)      │input events        │
└─────────────────┼───────────────────┼────────────────────┘
                  ▼                   │
┌──────────────────────────────────────────────────────────┐
│                     Termux Layer                         │
│                                                          │
│  ┌───────────────────────────────────────────────────┐  │
│  │              Waymux Bridge (Rust)                 │  │
│  │  • wlr-screencopy / ext-image-capture-source      │  │
│  │  • Frame encoder (zstd / H.264 / raw)             │  │
│  │  • WIP deserializer → Wayland input injection     │  │
│  └──────────────────┬────────────────────────────────┘  │
│                     │  $WAYLAND_DISPLAY socket           │
│                     ▼                                    │
│  ┌───────────────────────────────────────────────────┐  │
│  │          Light Speed Desktop (Rust)               │  │
│  │  • Smithay compositor (Wayland server)            │  │
│  │  • Hybrid stack/tile WM                           │  │
│  │  • Iced/GTK4 DE chrome                            │  │
│  │  • XWayland support                               │  │
│  └───────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

---

## 4. Communication Protocols

### 4.1 Transport

All communication between Waymux Client and Waymux Bridge uses a **Unix domain socket** exposed at `$TMPDIR/waymux.sock` (Termux-side) and accessed from Android via the shared `/data/data/com.termux/files/usr/tmp/` path (requires `sharedUserId` or Termux API plugin permissions).

The socket carries a **length-prefixed binary framing** protocol:

```
┌──────────┬────────────────┐
│ u32 len  │  message bytes │
└──────────┴────────────────┘
```

All multi-byte integers are little-endian.

### 4.2 Waymux Frame Protocol (WFP)

Messages from Bridge → Client:

| Message Type | ID | Payload |
|---|---|---|
| `FrameFull` | 0x01 | width, height, encoding_id, frame_data |
| `FrameDamage` | 0x02 | damage_rect[], encoding_id, frame_data |
| `DisplayInfo` | 0x03 | width, height, scale_factor, refresh_hz |
| `Ping` | 0x10 | sequence: u64 |
| `Disconnect` | 0xFF | reason_code: u8 |

Supported frame encodings:
- `RAW_BGRA8` (0x00): uncompressed, 4 bytes/pixel
- `ZSTD_BGRA8` (0x01): zstd-compressed raw frames
- `H264_ANNEXB` (0x02): H.264 Annex B bitstream (future)

### 4.3 Waymux Input Protocol (WIP)

Messages from Client → Bridge:

| Message Type | ID | Key Fields |
|---|---|---|
| `PointerMotion` | 0x01 | x: f32, y: f32, time_ms: u32 |
| `PointerButton` | 0x02 | button: u32, state: ButtonState, time_ms: u32 |
| `PointerAxis` | 0x03 | axis: Axis, value: f32, time_ms: u32 |
| `TouchDown` | 0x04 | id: u32, x: f32, y: f32, time_ms: u32 |
| `TouchMotion` | 0x05 | id: u32, x: f32, y: f32, time_ms: u32 |
| `TouchUp` | 0x06 | id: u32, time_ms: u32 |
| `StylusDown` | 0x07 | x: f32, y: f32, pressure: f32, tilt_x: f32, tilt_y: f32, time_ms: u32 |
| `StylusMotion` | 0x08 | x: f32, y: f32, pressure: f32, tilt_x: f32, tilt_y: f32, time_ms: u32 |
| `StylusUp` | 0x09 | time_ms: u32 |
| `KeyDown` | 0x0A | keycode: u32, modifiers: u32, time_ms: u32 |
| `KeyUp` | 0x0B | keycode: u32, modifiers: u32, time_ms: u32 |
| `Pong` | 0x10 | sequence: u64 |

Coordinates are normalized to the compositor's logical pixel space. The client must apply the inverse of any scaling applied for display.

---

## 5. Crate Dependency Map

```
waymux-proto          (shared types: WFP/WIP message enums, codecs)
    ├── waymux-bridge (uses waymux-proto + smithay-client-toolkit + zstd)
    └── waymux-client-rs (uses waymux-proto + wgpu; compiled as .so for Android JNI)

light-speed-desktop   (uses smithay + iced/gtk4; independent of waymux-proto)
```

---

## 6. Monorepo Structure

```
waymux/
├── SPEC.md                          ← this file
├── AGENTS.md                        ← AI coding assistant instructions
├── Cargo.toml                       ← workspace root
├── docs/
│   └── adr/                         ← Architecture Decision Records
│       ├── ADR-001-transport.md
│       ├── ADR-002-frame-encoding.md
│       ├── ADR-003-compositor-library.md
│       ├── ADR-004-android-rendering.md
│       └── ADR-005-widget-toolkit.md
├── waymux-proto/
│   ├── SPEC.md
│   └── src/
├── waymux-bridge/
│   ├── SPEC.md
│   └── src/
├── waymux-client-rs/
│   ├── SPEC.md
│   └── src/
├── waymux-client-android/           ← Android Studio project
│   ├── SPEC.md
│   ├── app/
│   └── build.gradle.kts
└── light-speed-desktop/
    ├── SPEC.md
    └── src/
```

---

## 7. Engineering Standards

### 7.1 Code Quality Rules (All Rust Crates)

- **File length limit:** 300 lines. Files approaching this limit must be split by logical concern.
- **No `unwrap()` or `expect()`** in library code. Use `?` propagation, typed errors (`thiserror`), or explicit `match`.
- **No `unsafe` blocks** unless a crate's purpose inherently requires FFI (e.g., `waymux-client-rs`'s JNI boundary). Every `unsafe` block must have a `// SAFETY:` comment explaining the invariants upheld.
- **No excessive `.clone()`**: prefer borrows, `Arc` sharing, or redesigned ownership. Clones that cross an allocation boundary for large data (frames, strings > 64 bytes) must be justified in a comment.
- **Rust 2024 edition** for all crates.
- **Apache-2.0** license for all crates except where a dual MIT/Apache-2.0 is needed for broader compatibility (document in ADR).

### 7.2 Test-Driven Development

- Write tests before or alongside implementation, never after.
- Each public function or method must have at least one unit test.
- Integration tests live in `tests/` at the crate root.
- Protocol encode/decode round-trip tests are mandatory in `waymux-proto`.
- Use `#[cfg(test)]` modules inline for unit tests.
- CI must run `cargo test --all-features` and `cargo test --no-default-features` for all crates.

### 7.3 Documentation

- All public items (`pub fn`, `pub struct`, `pub enum`, `pub trait`) must have rustdoc comments (`///`).
- Module-level docs (`//!`) are required for every `mod.rs` or crate root.
- Non-obvious implementation choices must have inline `//` comments explaining *why*, not *what*.
- Run `cargo doc --no-deps --all-features` without warnings in CI.

### 7.4 Error Handling

- Define crate-local error types using `thiserror`.
- Use `color-eyre` or `anyhow` only in binary entry points (`main.rs`), not in library code.
- Errors must be descriptive enough to diagnose without a debugger.

### 7.5 Async Runtime

- Use `tokio` as the async runtime for `waymux-bridge` and `waymux-client-rs`.
- Smithay's event loop (`calloop`) is used for compositor event handling in `light-speed-desktop`; async tasks that cross into tokio must use `calloop`'s tokio integration or channels.

---

## 8. Key Dependencies (Rust)

| Crate | Version | Purpose |
|---|---|---|
| `smithay` | 0.7 | Wayland compositor primitives (Bridge + LSD) |
| `smithay-client-toolkit` | 0.19 | Wayland client (Bridge) |
| `wayland-client` | 0.31 | Low-level Wayland client bindings |
| `wgpu` | 0.20 | GPU rendering in Android client |
| `zstd` | 0.13 | Frame compression |
| `tokio` | 1 | Async runtime |
| `thiserror` | 1 | Error type derivation |
| `serde` | 1 | Optional: config serialization |
| `tracing` | 0.1 | Structured logging |
| `iced` | 0.12 | DE UI chrome (Light Speed Desktop) |
| `jni` | 0.21 | JNI bindings for Android |

---

## 9. Platform Targets

| Sub-project | Target Triple |
|---|---|
| `waymux-bridge` | `aarch64-linux-android` (Termux) |
| `waymux-client-rs` | `aarch64-linux-android` (Android JNI `.so`) |
| `light-speed-desktop` | `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-gnu` |
| Android app | `arm64-v8a` (primary), `x86_64` (emulator) |

---

## 10. Milestones

| Milestone | Deliverable |
|---|---|
| M0 | Monorepo scaffold, all SPEC/AGENTS/ADR documents, CI skeleton |
| M1 | `waymux-proto` complete with round-trip tests |
| M2 | `waymux-bridge` connects to a running compositor, streams raw frames |
| M3 | `waymux-client` renders received frames on Android `SurfaceView` |
| M4 | Full bidirectional input: touch, keyboard, stylus |
| M5 | `waymux-bridge` zstd frame compression, damage region support |
| M6 | Light Speed Desktop: basic compositor boots, single-window stacking mode |
| M7 | Light Speed Desktop: tiling mode + responsive breakpoints |
| M8 | End-to-end demo: LSD running in Termux, displayed on Android via Waymux |
| M9 | Light Speed Desktop: Iced DE chrome (panel, launcher) |
| M10 | XWayland support in Light Speed Desktop |
