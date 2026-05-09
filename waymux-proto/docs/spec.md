# waymux-proto — Package Specification

**Crate:** `waymux-proto`  
**Type:** Library (`lib`)  
**Version:** 0.1.0  
**License:** Apache-2.0

---

## Purpose

`waymux-proto` is the single source of truth for all binary protocol types shared between `waymux-bridge` and `waymux-client-rs`. It defines, encodes, and decodes all messages in:

- **Waymux Frame Protocol (WFP):** messages flowing from Bridge to Client (frame data, display metadata).
- **Waymux Input Protocol (WIP):** messages flowing from Client to Bridge (pointer, touch, stylus, keyboard events).

This crate has **no runtime**, **no async**, and **no platform-specific code**. It must compile for both `aarch64-linux-android` and desktop targets without conditional compilation.

---

## Dependencies

```toml
[dependencies]
thiserror  = "1"
bytes      = "1"          # BytesMut/Bytes for zero-copy codec
serde      = { version = "1", features = ["derive"], optional = true }

[dev-dependencies]
proptest   = "1"
```

No `smithay`, `tokio`, `wgpu`, or `jni` dependencies are permitted in this crate.

---

## Module Layout

```
waymux-proto/src/
├── lib.rs          # crate root: re-exports all public types
├── wfp.rs          # WFP message types (Bridge → Client)
├── wip.rs          # WIP message types (Client → Bridge)
├── codec.rs        # Framing: length-prefix encode/decode
├── encoding.rs     # FrameEncoding enum + associated constants
└── error.rs        # CodecError type
```

---

## Public API

### `encoding.rs`

```rust
/// Identifies the encoding format of a frame payload.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameEncoding {
    /// Uncompressed BGRA8, 4 bytes per pixel, row-major.
    RawBgra8 = 0x00,
    /// Zstd-compressed BGRA8.
    ZstdBgra8 = 0x01,
    /// H.264 Annex B bitstream (reserved for future use).
    H264AnnexB = 0x02,
}
```

### `wfp.rs` — Bridge → Client

```rust
/// A complete message from the Waymux Bridge to the Waymux Client.
#[repr(u8)]
#[derive(Debug)]
pub enum WfpMessage {
    /// Full frame update covering the entire display area.
    FrameFull(FrameFullMsg) = 0x01,
    /// Partial frame update covering one or more damage rectangles.
    FrameDamage(FrameDamageMsg) = 0x02,
    /// Display geometry notification (sent on connect and on resize).
    DisplayInfo(DisplayInfoMsg) = 0x03,
    /// Keepalive ping; client must respond with WIP Pong.
    Ping { sequence: u64 } = 0x10,
    /// Server-initiated disconnect with a reason code.
    Disconnect { reason: DisconnectReason } = 0xFF,
}

pub struct FrameFullMsg {
    pub width: u32,
    pub height: u32,
    pub encoding: FrameEncoding,
    pub data: bytes::Bytes,
}

pub struct FrameDamageMsg {
    pub encoding: FrameEncoding,
    pub regions: Vec<DamageRegion>,
    pub data: bytes::Bytes,
}

pub struct DisplayInfoMsg {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub refresh_hz: f32,
}

pub struct DamageRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    ServerShutdown = 0x00,
    ProtocolError = 0x01,
    CompositorLost = 0x02,
}
```

### `wip.rs` — Client → Bridge

```rust
/// A complete message from the Waymux Client to the Waymux Bridge.
#[repr(u8)]
#[derive(Debug)]
pub enum WipMessage {
    PointerMotion(PointerMotionMsg) = 0x01,
    PointerButton(PointerButtonMsg) = 0x02,
    PointerAxis(PointerAxisMsg) = 0x03,
    TouchDown(TouchPointMsg) = 0x04,
    TouchMotion(TouchPointMsg) = 0x05,
    TouchUp { id: u32, time_ms: u32 } = 0x06,
    StylusDown(StylusMsg) = 0x07,
    StylusMotion(StylusMsg) = 0x08,
    StylusUp { time_ms: u32 } = 0x09,
    KeyDown(KeyMsg) = 0x0A,
    KeyUp(KeyMsg) = 0x0B,
    Pong { sequence: u64 } = 0x10,
}

pub struct PointerMotionMsg { pub x: f32, pub y: f32, pub time_ms: u32 }
pub struct PointerButtonMsg { pub button: u32, pub state: ButtonState, pub time_ms: u32 }
pub struct PointerAxisMsg   { pub axis: PointerAxis, pub value: f32, pub time_ms: u32 }
pub struct TouchPointMsg    { pub id: u32, pub x: f32, pub y: f32, pub time_ms: u32 }
pub struct StylusMsg        { pub x: f32, pub y: f32, pub pressure: f32,
                              pub tilt_x: f32, pub tilt_y: f32, pub time_ms: u32 }
pub struct KeyMsg           { pub keycode: u32, pub modifiers: u32, pub time_ms: u32 }

#[repr(u8)]
pub enum ButtonState { Released = 0, Pressed = 1 }

#[repr(u8)]
pub enum PointerAxis { Vertical = 0, Horizontal = 1 }
```

### `codec.rs` — Framing

```rust
/// Encodes a message into a length-prefixed frame appended to `buf`.
///
/// Frame format: [u32 little-endian payload length][payload bytes]
pub fn encode_wfp(msg: &WfpMessage, buf: &mut bytes::BytesMut) -> Result<(), CodecError>;
pub fn encode_wip(msg: &WipMessage, buf: &mut bytes::BytesMut) -> Result<(), CodecError>;

/// Attempts to decode one WFP message from the front of `buf`.
///
/// Returns `Ok(None)` if more bytes are needed (partial frame).
pub fn decode_wfp(buf: &mut bytes::BytesMut) -> Result<Option<WfpMessage>, CodecError>;
pub fn decode_wip(buf: &mut bytes::BytesMut) -> Result<Option<WipMessage>, CodecError>;
```

---

## Testing Requirements

- `tests/round_trip.rs`: encode then decode every variant of `WfpMessage` and `WipMessage` and assert equality.
- `tests/proptest_codec.rs`: use `proptest` strategies to generate random field values for each message type and verify round-trip integrity.
- `tests/partial_frame.rs`: simulate partial byte delivery by splitting encoded frames at various offsets and verify `decode_wfp` returns `Ok(None)` until the full frame arrives.
- All tests run with `--no-default-features` (no serde) and `--all-features` (with serde).

---

## Codec Binary Layout (detailed)

Each encoded message starts with its type discriminant (`u8`) followed by type-specific fields, all little-endian:

### WFP FrameFull (0x01)
```
[0x01][width: u32][height: u32][encoding: u8][data_len: u32][data: bytes]
```

### WFP FrameDamage (0x02)
```
[0x02][encoding: u8][region_count: u16]
  for each region: [x: u32][y: u32][width: u32][height: u32]
[data_len: u32][data: bytes]
```

### WFP DisplayInfo (0x03)
```
[0x03][width: u32][height: u32][scale: f32][refresh: f32]
```

### WFP Ping (0x10)
```
[0x10][sequence: u64]
```

### WFP Disconnect (0xFF)
```
[0xFF][reason: u8]
```

### WIP PointerMotion (0x01)
```
[0x01][x: f32][y: f32][time_ms: u32]
```

### WIP TouchDown/Motion (0x04/0x05)
```
[id][x: f32][y: f32][time_ms: u32]
```

### WIP StylusDown/Motion (0x07/0x08)
```
[x: f32][y: f32][pressure: f32][tilt_x: f32][tilt_y: f32][time_ms: u32]
```

### WIP KeyDown/Up (0x0A/0x0B)
```
[keycode: u32][modifiers: u32][time_ms: u32]
```
