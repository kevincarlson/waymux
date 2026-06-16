//! `waymux-client` is the host-portable core of the Waymux Android client: the
//! Unix-socket transport, WFP frame decoding, WIP input serialization, and the
//! [`ClientState`] machine that ties them together.
//!
//! The Android-specific layers — the JNI cdylib exports and the wgpu renderer
//! that blits decoded frames onto an `ANativeWindow` — are added together with
//! the Android Studio project; they require the Android NDK to build. This
//! crate deliberately contains only logic that builds and is tested on the
//! host, so the protocol, decoding, and input paths can be verified in CI.
//!
//! Typical use from the (future) JNI layer:
//!
//! 1. [`ClientState::connect`] to the bridge socket.
//! 2. Poll [`ClientState::latest_frame`] each vsync and upload it to a texture.
//! 3. Build WIP messages with [`InputSerializer`] and forward them via
//!    [`ClientState::send_input`].

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod connection;
pub mod decoder;
pub mod error;
pub mod input;
pub mod state;

pub use decoder::{DecodedFrame, decode_full};
pub use error::ClientError;
pub use input::{InputSerializer, StylusInput, serialize};
pub use state::ClientState;
