//! Waymux Input Protocol (WIP) message types: Client → Bridge.
//!
//! This module defines the message types; their binary encode/decode lives in
//! [`codec`]. Coordinates are logical pixels in compositor space.

mod codec;

use crate::error::CodecError;

/// A complete message from the Waymux Client to the Waymux Bridge.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WipMessage {
    /// Absolute pointer motion.
    PointerMotion(PointerMotionMsg),
    /// Pointer button press or release.
    PointerButton(PointerButtonMsg),
    /// Pointer scroll/axis event.
    PointerAxis(PointerAxisMsg),
    /// A touch point made contact.
    TouchDown(TouchPointMsg),
    /// A touch point moved while in contact.
    TouchMotion(TouchPointMsg),
    /// A touch point lifted.
    TouchUp {
        /// Identifier of the touch point that lifted.
        id: u32,
        /// Event timestamp in milliseconds.
        time_ms: u32,
    },
    /// A stylus made contact.
    StylusDown(StylusMsg),
    /// A stylus moved while in contact.
    StylusMotion(StylusMsg),
    /// A stylus lifted.
    StylusUp {
        /// Event timestamp in milliseconds.
        time_ms: u32,
    },
    /// A key was pressed.
    KeyDown(KeyMsg),
    /// A key was released.
    KeyUp(KeyMsg),
    /// Response to a WFP `Ping`.
    Pong {
        /// Sequence number copied from the ping.
        sequence: u64,
    },
}

/// Body of [`WipMessage::PointerMotion`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointerMotionMsg {
    /// X coordinate in logical pixels.
    pub x: f32,
    /// Y coordinate in logical pixels.
    pub y: f32,
    /// Event timestamp in milliseconds.
    pub time_ms: u32,
}

/// Body of [`WipMessage::PointerButton`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointerButtonMsg {
    /// Linux input button code (e.g. `BTN_LEFT`).
    pub button: u32,
    /// Whether the button was pressed or released.
    pub state: ButtonState,
    /// Event timestamp in milliseconds.
    pub time_ms: u32,
}

/// Body of [`WipMessage::PointerAxis`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointerAxisMsg {
    /// Which axis scrolled.
    pub axis: PointerAxis,
    /// Scroll delta; sign follows the Wayland convention.
    pub value: f32,
    /// Event timestamp in milliseconds.
    pub time_ms: u32,
}

/// Body shared by [`WipMessage::TouchDown`] and [`WipMessage::TouchMotion`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TouchPointMsg {
    /// Identifier of this touch point within the active gesture.
    pub id: u32,
    /// X coordinate in logical pixels.
    pub x: f32,
    /// Y coordinate in logical pixels.
    pub y: f32,
    /// Event timestamp in milliseconds.
    pub time_ms: u32,
}

/// Body shared by [`WipMessage::StylusDown`] and [`WipMessage::StylusMotion`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StylusMsg {
    /// X coordinate in logical pixels.
    pub x: f32,
    /// Y coordinate in logical pixels.
    pub y: f32,
    /// Normalised pressure in `0.0..=1.0`.
    pub pressure: f32,
    /// Tilt along the X axis in radians.
    pub tilt_x: f32,
    /// Tilt along the Y axis in radians.
    pub tilt_y: f32,
    /// Event timestamp in milliseconds.
    pub time_ms: u32,
}

/// Body shared by [`WipMessage::KeyDown`] and [`WipMessage::KeyUp`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyMsg {
    /// Linux input keycode.
    pub keycode: u32,
    /// Active modifier bitmask.
    pub modifiers: u32,
    /// Event timestamp in milliseconds.
    pub time_ms: u32,
}

/// Whether a pointer button is pressed or released.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ButtonState {
    /// Button is up.
    Released = 0,
    /// Button is down.
    Pressed = 1,
}

impl ButtonState {
    /// Returns the wire discriminant byte.
    #[must_use]
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Parses a wire discriminant byte.
    ///
    /// # Errors
    /// Returns [`CodecError::UnknownButtonState`] for an unknown value.
    pub fn from_u8(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::Released),
            1 => Ok(Self::Pressed),
            other => Err(CodecError::UnknownButtonState(other)),
        }
    }
}

/// Which pointer scroll axis an event applies to.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PointerAxis {
    /// Vertical scrolling.
    Vertical = 0,
    /// Horizontal scrolling.
    Horizontal = 1,
}

impl PointerAxis {
    /// Returns the wire discriminant byte.
    #[must_use]
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Parses a wire discriminant byte.
    ///
    /// # Errors
    /// Returns [`CodecError::UnknownPointerAxis`] for an unknown value.
    pub fn from_u8(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::Vertical),
            1 => Ok(Self::Horizontal),
            other => Err(CodecError::UnknownPointerAxis(other)),
        }
    }
}
