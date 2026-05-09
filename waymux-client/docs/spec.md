# Waymux Client — Package Specification

This document covers both sub-packages:
- `waymux-client-rs/` — Rust JNI library (`cdylib`)
- `waymux-client-android/` — Android application (Kotlin)

**Version:** 0.1.0  
**License:** Apache-2.0  
**Android min SDK:** 29 (Android 10)  
**Target SDK:** 35 (Android 15)  
**Rust target:** `aarch64-linux-android`

---

## Purpose

The Waymux Client is an Android application that:
1. Connects to the Waymux Bridge running in Termux over a Unix domain socket.
2. Receives encoded frame buffers and renders them at full resolution using a `SurfaceView` + wgpu (Vulkan backend).
3. Captures all Android input events (touch, stylus, keyboard, mouse) and forwards them as WIP messages to the bridge.

The Kotlin `Activity` is a thin shell that manages Android lifecycle and delegates all logic to `libwaymux_client.so` (the Rust JNI library).

---

## Architecture

```
┌─────────────────────────────────────────┐
│         MainActivity (Kotlin)           │
│  • SurfaceView creation & lifecycle     │
│  • Permission requests (Termux access)  │
│  • Input event capture & forwarding     │
│  • Calls JNI exports in libwaymux_client│
└──────────────┬──────────────────────────┘
               │ JNI
               ▼
┌─────────────────────────────────────────┐
│       libwaymux_client.so (Rust)        │
│  ┌──────────────────────────────────┐   │
│  │ connection/   tokio UnixStream   │   │
│  │ decoder/      WFP frame decode   │   │
│  │ renderer/     wgpu SurfaceView   │   │
│  │ input/        WIP serialization  │   │
│  └──────────────────────────────────┘   │
└─────────────────────────────────────────┘
```

---

## waymux-client-rs

### Dependencies

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
waymux-proto   = { path = "../waymux-proto" }
wgpu           = { version = "0.20", features = ["vulkan"] }
tokio          = { version = "1", features = ["full"] }
bytes          = "1"
thiserror      = "1"
tracing        = "0.1"
tracing-android = "0.2"   # routes tracing to Android logcat
jni            = "0.21"
zstd           = "0.13"
```

### Module Layout

```
waymux-client-rs/src/
├── lib.rs            # JNI exports only; initializes runtime on first call
├── error.rs          # ClientError type
├── state.rs          # ClientState: owns runtime, renderer, connection
├── connection/
│   ├── mod.rs        # Connection trait
│   └── unix.rs       # tokio UnixStream client; WFP read + WIP write
├── decoder/
│   ├── mod.rs        # FrameDecoder trait
│   ├── raw.rs        # Passthrough for RawBgra8
│   └── zstd.rs       # Zstd decompression → BGRA8
├── renderer/
│   ├── mod.rs        # Renderer: wgpu Device, Queue, Surface, Pipeline
│   ├── pipeline.rs   # Render pipeline: BGRA8 texture upload → fullscreen blit
│   └── texture.rs    # TextureHandle: create, update, bind
└── input/
    ├── mod.rs        # InputSerializer: Android MotionEvent → WipMessage
    ├── pointer.rs    # Mouse / trackpad events
    ├── touch.rs      # Multi-touch events
    ├── stylus.rs     # Stylus / pen events (Android S_PEN_TOOL_TYPE_STYLUS)
    └── keyboard.rs   # KeyEvent → WIP KeyDown/Up
```

### JNI Exports (lib.rs)

All JNI function names follow the pattern `Java_app_appthere_waymux_RustBridge_<method>`.

```rust
/// Initialize the Rust runtime and create a ClientState.
/// Returns an opaque pointer (jlong) to the heap-allocated state.
/// Must be called once before any other JNI function.
#[no_mangle]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_init(
    env: JNIEnv,
    _class: JClass,
    socket_path: JString,
) -> jlong;

/// Provide the ANativeWindow from the Kotlin SurfaceHolder.
/// Must be called after surfaceCreated().
#[no_mangle]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_surfaceCreated(
    env: JNIEnv,
    _class: JClass,
    state_ptr: jlong,
    surface: JObject,
);

/// Notify of surface resize.
#[no_mangle]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_surfaceChanged(
    env: JNIEnv,
    _class: JClass,
    state_ptr: jlong,
    width: jint,
    height: jint,
);

/// Notify that the surface is being destroyed. Must pause rendering.
#[no_mangle]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_surfaceDestroyed(
    _env: JNIEnv,
    _class: JClass,
    state_ptr: jlong,
);

/// Forward a serialized WIP message byte array to the bridge.
/// Called from Kotlin input event handlers.
#[no_mangle]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_sendInputEvent(
    env: JNIEnv,
    _class: JClass,
    state_ptr: jlong,
    event_type: jint,
    payload: jbyteArray,
);

/// Release the ClientState. Must be called in onDestroy().
#[no_mangle]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_destroy(
    _env: JNIEnv,
    _class: JClass,
    state_ptr: jlong,
);
```

### Rendering Pipeline

1. On `surfaceCreated`, obtain `ANativeWindow` from `Surface` via `ANativeWindow_fromSurface` (called from Rust via a `// SAFETY:`-documented unsafe block).
2. Create `wgpu::Instance`, `wgpu::Surface`, request `Adapter` (Vulkan backend preferred), create `Device` and `Queue`.
3. Configure surface with `Bgra8Unorm` or `Rgba8Unorm` format matching the WFP frame format.
4. On each decoded frame: upload pixel data to a `wgpu::Texture` (via `Queue::write_texture`), execute a fullscreen blit render pass, call `surface.present()`.
5. Rendering runs on the tokio runtime's blocking thread pool to avoid blocking the JNI caller.

### Input Coordinate Normalization

Android `MotionEvent` coordinates are in display pixels. The client maintains the compositor's logical resolution (received via `WfpMessage::DisplayInfo`). Before serializing:

```
compositor_x = android_x * (compositor_width  / surface_width)
compositor_y = android_y * (compositor_height / surface_height)
```

These normalized coordinates are embedded in WIP messages.

---

## waymux-client-android (Kotlin)

### Key Files

```
waymux-client-android/app/src/main/
├── kotlin/app/appthere/waymux/
│   ├── MainActivity.kt       # Activity: lifecycle, SurfaceView, input
│   ├── RustBridge.kt         # JNI declaration object
│   ├── InputForwarder.kt     # MotionEvent → RustBridge.sendInputEvent()
│   └── PermissionHelper.kt   # Runtime permission flow (Termux access)
├── res/
│   └── layout/activity_main.xml   # Single SurfaceView, full-screen
└── AndroidManifest.xml
```

### RustBridge.kt

```kotlin
object RustBridge {
    init { System.loadLibrary("waymux_client") }

    external fun init(socketPath: String): Long
    external fun surfaceCreated(statePtr: Long, surface: Surface)
    external fun surfaceChanged(statePtr: Long, width: Int, height: Int)
    external fun surfaceDestroyed(statePtr: Long)
    external fun sendInputEvent(statePtr: Long, eventType: Int, payload: ByteArray)
    external fun destroy(statePtr: Long)
}
```

### MainActivity.kt Responsibilities

- On `onCreate`: call `RustBridge.init(socketPath)` with the resolved socket path.
- On `surfaceCreated/Changed/Destroyed`: delegate to `RustBridge`.
- Override `onTouchEvent`, `onGenericMotionEvent`, `onKeyDown`, `onKeyUp`: serialize via `InputForwarder` and call `RustBridge.sendInputEvent`.
- On `onDestroy`: call `RustBridge.destroy(statePtr)`.
- Keep the screen on while connected (`WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON`).

### Socket Path Resolution

The bridge socket is at `$TMPDIR/waymux.sock` in Termux. From Android, this maps to `/data/data/com.termux/files/usr/tmp/waymux.sock`. The app must have the `com.termux.permission.RUN_COMMAND` permission or share a `sharedUserId` with Termux (requires both apps to be signed with the same key). The socket path is user-configurable via the app's Settings screen.

### Permissions Required

```xml
<uses-permission android:name="com.termux.permission.RUN_COMMAND" />
```

No internet permission is required (all communication is local Unix socket).

---

## Testing Requirements

### Rust (waymux-client-rs)

- `tests/decoder_raw.rs`: create a synthetic `WfpMessage::FrameFull` with `RawBgra8` encoding, decode it, verify pixel data matches.
- `tests/decoder_zstd.rs`: same with `ZstdBgra8` encoding; compress input first with `zstd`, then decode.
- `tests/input_touch.rs`: create synthetic touch coordinate pairs, serialize to WIP, decode with `waymux-proto`, verify coordinate normalization.
- `tests/input_stylus.rs`: verify stylus pressure, tilt fields survive serialization round-trip.
- Note: wgpu rendering tests are excluded from unit tests (require a display). Use a headless wgpu adapter in CI where available.

### Android (Kotlin)

- Instrumented tests using `ActivityScenario` to verify `RustBridge` load does not crash.
- Mock `RustBridge` for unit tests of `InputForwarder` logic.
