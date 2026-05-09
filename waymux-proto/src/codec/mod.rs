// SPDX-License-Identifier: Apache-2.0

//! Codec for length-prefixed WFP and WIP frames.
//!
//! Each frame on the wire is:
//! ```text
//! ┌──────────────────┬───────────────────────────────┐
//! │ u32 LE len       │ payload (len bytes)            │
//! └──────────────────┴───────────────────────────────┘
//! ```
//! The payload starts with a one-byte message-type discriminant followed by
//! type-specific fields, all in little-endian byte order.

mod decode;
mod encode;

pub use decode::{decode_wfp, decode_wip};
pub use encode::{encode_wfp, encode_wip};
