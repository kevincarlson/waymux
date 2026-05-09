// SPDX-License-Identifier: Apache-2.0

//! Waymux Client Rust library: JNI-exported functions for frame decoding via
//! wgpu and input event serialization to the Waymux Input Protocol (WIP).
//!
//! # Safety policy
//!
//! This crate requires one `unsafe` block at the JNI boundary in this file.
//! All other modules use `#![forbid(unsafe_code)]`. Any future JNI exports
//! must be added here with a `// SAFETY:` comment documenting the invariants
//! upheld by the Android runtime calling convention.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(missing_docs)]
#![warn(clippy::clone_on_ref_ptr)]
// unsafe_code is intentionally NOT forbidden at the crate root because the
// JNI boundary requires it. All internal modules must carry
// `#![forbid(unsafe_code)]` individually.
