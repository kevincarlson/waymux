// SPDX-License-Identifier: Apache-2.0

//! `waymux-proto` — shared binary protocol types and framing codec for the
//! Waymux system.
//!
//! This crate provides:
//! - **WFP** ([`WfpMessage`]): messages from the Waymux Bridge to the Client.
//! - **WIP** ([`WipMessage`]): messages from the Waymux Client to the Bridge.
//! - A length-prefixed binary codec ([`encode_wfp`], [`decode_wfp`], etc.)
//!   built on [`bytes::BytesMut`] for zero-copy framing.
//!
//! This crate has no async runtime dependency and compiles for both
//! `aarch64-linux-android` and desktop targets.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(unsafe_code)]
#![deny(missing_docs)]
#![warn(clippy::clone_on_ref_ptr)]

pub mod codec;
pub mod encoding;
pub mod error;
pub mod wfp;
pub mod wip;

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
