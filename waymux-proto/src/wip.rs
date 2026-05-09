// SPDX-License-Identifier: Apache-2.0

//! Waymux Input Protocol (WIP) message types: Client → Bridge.

use crate::error::CodecError;

/// A complete message sent from the Waymux Client to the Waymux Bridge.
#[repr(u8)]
#[derive(Debug, Clone)]
pub enum WipMessage {
    /// Mouse or trackpad pointer movement in logical pixels.
    PointerMotion(PointerMotionMsg) = 0x01,
    /// Mouse or trackpad button press or release.
    PointerButton(PointerButtonMsg) = 0x02,
    /// Mouse scroll wheel or trackpad scroll gesture.
    PointerAxis(PointerAxisMsg) = 0x03,
    /// Touch contact started.
    TouchDown(TouchPointMsg) = 0x04,
    /// Touch contact moved.
    TouchMotion(TouchPointMsg) = 0x05,
    /// Touch contact lifted.
    TouchUp {
        /// Touch contact identifier, matching a prior [`WipMessage::TouchDown`].
        id: u32,
        /// Compositor time in milliseconds.
        time_ms: u32,
    } = 0x06,
    /// Stylus tip contacted the surface.
    StylusDown(StylusMsg) = 0x07,
    /// Stylus moved while in contact with the surface.
    StylusMotion(StylusMsg) = 0x08,
    /// Stylus tip lifted from the surface.
    StylusUp {
        /// Compositor time in milliseconds.
        time_ms: u32,
    } = 0x09,
    /// Physical key pressed.
    KeyDown(KeyMsg) = 0x0A,
    /// Physical key released.
    KeyUp(KeyMsg) = 0x0B,
    /// Response to a [`crate::WfpMessage::Ping`] keepalive.
    Pong {
        /// Sequence number copied from the corresponding Ping message.
        sequence: u64,
    } = 0x10,
}

impl PartialEq for WipMessage {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (WipMessage::PointerMotion(a), WipMessage::PointerMotion(b)) => a == b,
            (WipMessage::PointerButton(a), WipMessage::PointerButton(b)) => a == b,
            (WipMessage::PointerAxis(a), WipMessage::PointerAxis(b)) => a == b,
            (WipMessage::TouchDown(a), WipMessage::TouchDown(b)) => a == b,
            (WipMessage::TouchMotion(a), WipMessage::TouchMotion(b)) => a == b,
            (WipMessage::TouchUp { id: a_id, time_ms: a_t }, WipMessage::TouchUp { id: b_id, time_ms: b_t }) => {
                a_id == b_id && a_t == b_t
            }
            (WipMessage::StylusDown(a), WipMessage::StylusDown(b)) => a == b,
            (WipMessage::StylusMotion(a), WipMessage::StylusMotion(b)) => a == b,
            (WipMessage::StylusUp { time_ms: a }, WipMessage::StylusUp { time_ms: b }) => a == b,
            (WipMessage::KeyDown(a), WipMessage::KeyDown(b)) => a == b,
            (WipMessage::KeyUp(a), WipMessage::KeyUp(b)) => a == b,
            (WipMessage::Pong { sequence: a }, WipMessage::Pong { sequence: b }) => a == b,
            _ => false,
        }
    }
}

/// Payload for [`WipMessage::PointerMotion`].
#[derive(Debug, Clone, PartialEq)]
pub struct PointerMotionMsg {
    /// Horizontal position in compositor logical pixels.
    pub x: f32,
    /// Vertical position in compositor logical pixels.
    pub y: f32,
    /// Compositor time in milliseconds.
    pub time_ms: u32,
}

/// Payload for [`WipMessage::PointerButton`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointerButtonMsg {
    /// Linux input event code (e.g. `BTN_LEFT = 0x110`).
    pub button: u32,
    /// Whether the button was pressed or released.
    pub state: ButtonState,
    /// Compositor time in milliseconds.
    pub time_ms: u32,
}

/// Payload for [`WipMessage::PointerAxis`].
#[derive(Debug, Clone, PartialEq)]
pub struct PointerAxisMsg {
    /// Which axis scrolled.
    pub axis: PointerAxis,
    /// Scroll distance in logical pixels (positive = down/right).
    pub value: f32,
    /// Compositor time in milliseconds.
    pub time_ms: u32,
}

/// Payload for [`WipMessage::TouchDown`] and [`WipMessage::TouchMotion`].
#[derive(Debug, Clone, PartialEq)]
pub struct TouchPointMsg {
    /// Touch contact identifier assigned by the Android input system.
    pub id: u32,
    /// Horizontal position in compositor logical pixels.
    pub x: f32,
    /// Vertical position in compositor logical pixels.
    pub y: f32,
    /// Compositor time in milliseconds.
    pub time_ms: u32,
}

/// Payload for [`WipMessage::StylusDown`] and [`WipMessage::StylusMotion`].
#[derive(Debug, Clone, PartialEq)]
pub struct StylusMsg {
    /// Horizontal position in compositor logical pixels.
    pub x: f32,
    /// Vertical position in compositor logical pixels.
    pub y: f32,
    /// Normalized tip pressure in the range `[0.0, 1.0]`.
    pub pressure: f32,
    /// Tilt angle around the X axis in degrees.
    pub tilt_x: f32,
    /// Tilt angle around the Y axis in degrees.
    pub tilt_y: f32,
    /// Compositor time in milliseconds.
    pub time_ms: u32,
}

/// Payload for [`WipMessage::KeyDown`] and [`WipMessage::KeyUp`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyMsg {
    /// Linux key code (XKB key symbol).
    pub keycode: u32,
    /// Bitmask of active modifier keys.
    pub modifiers: u32,
    /// Compositor time in milliseconds.
    pub time_ms: u32,
}

/// Whether a pointer button or touch was pressed or released.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    /// The button was released.
    Released = 0,
    /// The button was pressed.
    Pressed = 1,
}

impl TryFrom<u8> for ButtonState {
    type Error = CodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ButtonState::Released),
            1 => Ok(ButtonState::Pressed),
            other => Err(CodecError::UnknownButtonState(other)),
        }
    }
}

/// Which axis a [`WipMessage::PointerAxis`] event refers to.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerAxis {
    /// Vertical scroll (wheel up/down, two-finger swipe up/down).
    Vertical = 0,
    /// Horizontal scroll (wheel tilt, two-finger swipe left/right).
    Horizontal = 1,
}

impl TryFrom<u8> for PointerAxis {
    type Error = CodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(PointerAxis::Vertical),
            1 => Ok(PointerAxis::Horizontal),
            other => Err(CodecError::UnknownPointerAxis(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_motion_construction() {
        let msg = PointerMotionMsg { x: 100.5, y: 200.0, time_ms: 12345 };
        let cloned = msg.clone();
        assert_eq!(cloned.x, 100.5);
    }

    #[test]
    fn pointer_button_construction() {
        let msg = PointerButtonMsg { button: 0x110, state: ButtonState::Pressed, time_ms: 999 };
        let cloned = msg.clone();
        assert_eq!(cloned.button, 0x110);
    }

    #[test]
    fn stylus_msg_construction() {
        let msg = StylusMsg {
            x: 1.0,
            y: 2.0,
            pressure: 0.5,
            tilt_x: -15.0,
            tilt_y: 10.0,
            time_ms: 500,
        };
        let cloned = msg.clone();
        assert_eq!(cloned.pressure, 0.5);
    }

    #[test]
    fn button_state_try_from_valid() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(ButtonState::try_from(0)?, ButtonState::Released);
        assert_eq!(ButtonState::try_from(1)?, ButtonState::Pressed);
        Ok(())
    }

    #[test]
    fn button_state_unknown_byte() {
        let result = ButtonState::try_from(2);
        assert!(matches!(result, Err(CodecError::UnknownButtonState(2))));
    }

    #[test]
    fn pointer_axis_try_from_valid() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(PointerAxis::try_from(0)?, PointerAxis::Vertical);
        assert_eq!(PointerAxis::try_from(1)?, PointerAxis::Horizontal);
        Ok(())
    }

    #[test]
    fn pointer_axis_unknown_byte() {
        let result = PointerAxis::try_from(5);
        assert!(matches!(result, Err(CodecError::UnknownPointerAxis(5))));
    }

    #[test]
    fn wip_message_partial_eq() {
        let a = WipMessage::Pong { sequence: 7 };
        let b = WipMessage::Pong { sequence: 7 };
        let c = WipMessage::StylusUp { time_ms: 0 };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
