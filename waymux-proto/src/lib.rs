//! `waymux-proto` defines the binary wire protocol shared by `waymux-bridge`
//! and the Waymux Android client.
//!
//! Two message families are defined:
//!
//! * [`WfpMessage`] — the Waymux Frame Protocol, flowing Bridge → Client
//!   (frame data and display metadata).
//! * [`WipMessage`] — the Waymux Input Protocol, flowing Client → Bridge
//!   (pointer, touch, stylus, and keyboard events).
//!
//! Both families share one length-prefixed framing: a little-endian `u32`
//! payload length followed by the message bytes. Use [`encode_wfp`] /
//! [`encode_wip`] to append a framed message to a buffer, and [`decode_wfp`] /
//! [`decode_wip`] to pull one message back out of a streaming buffer
//! (`Ok(None)` means more bytes are needed).
//!
//! The crate has no async runtime and no platform-specific code, so it builds
//! identically for Android and desktop targets. Enable the `serde` feature to
//! derive `Serialize`/`Deserialize` on every message type.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod codec;
mod encoding;
mod error;
mod wfp;
mod wip;

pub use codec::{decode_wfp, decode_wip, encode_wfp, encode_wip};
pub use encoding::FrameEncoding;
pub use error::CodecError;
pub use wfp::{
    DamageRegion, DisconnectReason, DisplayInfoMsg, FrameDamageMsg, FrameFullMsg, WfpMessage,
};
pub use wip::{
    ButtonState, KeyMsg, PointerAxis, PointerAxisMsg, PointerButtonMsg, PointerMotionMsg,
    StylusMsg, TouchPointMsg, WipMessage,
};
