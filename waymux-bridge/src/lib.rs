// SPDX-License-Identifier: Apache-2.0

//! Waymux Bridge library: reusable modules shared between the daemon binary
//! and integration tests.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(unsafe_code)]
#![deny(missing_docs)]
#![warn(clippy::clone_on_ref_ptr)]

pub mod compositor;
pub mod config;
pub mod encoder;
pub mod error;
pub mod pipeline;
pub mod server;
