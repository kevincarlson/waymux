//! Android input events → WIP messages, with display-space coordinate
//! normalization.
//!
//! Android `MotionEvent` coordinates are in surface (display) pixels; the
//! bridge expects compositor logical pixels. The serializer scales pointer,
//! touch, and stylus coordinates by `compositor / surface` per axis, matching
//! `waymux-client/docs/spec.md`. Button, axis, and key events carry no spatial
//! component and pass through unscaled.

use bytes::{Bytes, BytesMut};
use waymux_proto::{
    ButtonState, KeyMsg, PointerAxis, PointerAxisMsg, PointerButtonMsg, PointerMotionMsg,
    StylusMsg, TouchPointMsg, WipMessage, encode_wip,
};

use crate::error::ClientError;

/// Converts Android-space input into WIP messages in compositor space.
#[derive(Debug, Clone, Copy)]
pub struct InputSerializer {
    compositor_width: u32,
    compositor_height: u32,
    surface_width: u32,
    surface_height: u32,
}

impl Default for InputSerializer {
    fn default() -> Self {
        // Identity mapping until geometry is known; avoids divide-by-zero.
        Self {
            compositor_width: 1,
            compositor_height: 1,
            surface_width: 1,
            surface_height: 1,
        }
    }
}

impl InputSerializer {
    /// Creates a serializer with identity scaling.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the compositor's logical resolution (from `DisplayInfo`).
    pub fn set_compositor_size(&mut self, width: u32, height: u32) {
        self.compositor_width = width.max(1);
        self.compositor_height = height.max(1);
    }

    /// Sets the Android surface resolution (from `surfaceChanged`).
    pub fn set_surface_size(&mut self, width: u32, height: u32) {
        self.surface_width = width.max(1);
        self.surface_height = height.max(1);
    }

    /// Maps a surface-space coordinate into compositor logical space.
    #[must_use]
    pub fn normalize(&self, x: f32, y: f32) -> (f32, f32) {
        let sx = self.compositor_width as f32 / self.surface_width as f32;
        let sy = self.compositor_height as f32 / self.surface_height as f32;
        (x * sx, y * sy)
    }

    /// Pointer motion at surface coordinates `(x, y)`.
    #[must_use]
    pub fn pointer_motion(&self, x: f32, y: f32, time_ms: u32) -> WipMessage {
        let (x, y) = self.normalize(x, y);
        WipMessage::PointerMotion(PointerMotionMsg { x, y, time_ms })
    }

    /// Pointer button press or release.
    #[must_use]
    pub fn pointer_button(&self, button: u32, state: ButtonState, time_ms: u32) -> WipMessage {
        WipMessage::PointerButton(PointerButtonMsg {
            button,
            state,
            time_ms,
        })
    }

    /// Pointer scroll/axis event.
    #[must_use]
    pub fn pointer_axis(&self, axis: PointerAxis, value: f32, time_ms: u32) -> WipMessage {
        WipMessage::PointerAxis(PointerAxisMsg {
            axis,
            value,
            time_ms,
        })
    }

    /// Touch-down for point `id` at surface coordinates `(x, y)`.
    #[must_use]
    pub fn touch_down(&self, id: u32, x: f32, y: f32, time_ms: u32) -> WipMessage {
        WipMessage::TouchDown(self.touch_point(id, x, y, time_ms))
    }

    /// Touch-move for point `id`.
    #[must_use]
    pub fn touch_motion(&self, id: u32, x: f32, y: f32, time_ms: u32) -> WipMessage {
        WipMessage::TouchMotion(self.touch_point(id, x, y, time_ms))
    }

    /// Touch-up for point `id`.
    #[must_use]
    pub fn touch_up(&self, id: u32, time_ms: u32) -> WipMessage {
        WipMessage::TouchUp { id, time_ms }
    }

    fn touch_point(&self, id: u32, x: f32, y: f32, time_ms: u32) -> TouchPointMsg {
        let (x, y) = self.normalize(x, y);
        TouchPointMsg { id, x, y, time_ms }
    }

    /// Stylus contact at surface coordinates `(x, y)`.
    #[must_use]
    pub fn stylus_down(&self, s: StylusInput, time_ms: u32) -> WipMessage {
        WipMessage::StylusDown(self.stylus(s, time_ms))
    }

    /// Stylus motion while in contact.
    #[must_use]
    pub fn stylus_motion(&self, s: StylusInput, time_ms: u32) -> WipMessage {
        WipMessage::StylusMotion(self.stylus(s, time_ms))
    }

    /// Stylus lift.
    #[must_use]
    pub fn stylus_up(&self, time_ms: u32) -> WipMessage {
        WipMessage::StylusUp { time_ms }
    }

    fn stylus(&self, s: StylusInput, time_ms: u32) -> StylusMsg {
        let (x, y) = self.normalize(s.x, s.y);
        StylusMsg {
            x,
            y,
            pressure: s.pressure,
            tilt_x: s.tilt_x,
            tilt_y: s.tilt_y,
            time_ms,
        }
    }

    /// Key press.
    #[must_use]
    pub fn key_down(&self, keycode: u32, modifiers: u32, time_ms: u32) -> WipMessage {
        WipMessage::KeyDown(KeyMsg {
            keycode,
            modifiers,
            time_ms,
        })
    }

    /// Key release.
    #[must_use]
    pub fn key_up(&self, keycode: u32, modifiers: u32, time_ms: u32) -> WipMessage {
        WipMessage::KeyUp(KeyMsg {
            keycode,
            modifiers,
            time_ms,
        })
    }
}

/// Stylus sample in surface space (coordinates are normalized on serialize).
#[derive(Debug, Clone, Copy)]
pub struct StylusInput {
    /// X coordinate in surface pixels.
    pub x: f32,
    /// Y coordinate in surface pixels.
    pub y: f32,
    /// Normalised pressure in `0.0..=1.0`.
    pub pressure: f32,
    /// Tilt along the X axis in radians.
    pub tilt_x: f32,
    /// Tilt along the Y axis in radians.
    pub tilt_y: f32,
}

/// Serializes a WIP message into a length-prefixed frame ready for the socket.
///
/// # Errors
/// Returns [`ClientError::Codec`] if encoding fails.
pub fn serialize(msg: &WipMessage) -> Result<Bytes, ClientError> {
    let mut buf = BytesMut::new();
    encode_wip(msg, &mut buf)?;
    Ok(buf.freeze())
}

#[cfg(test)]
mod tests {
    use super::*;
    use waymux_proto::decode_wip;

    fn make() -> InputSerializer {
        let mut s = InputSerializer::new();
        s.set_compositor_size(1600, 1200);
        s.set_surface_size(800, 600);
        s
    }

    #[test]
    fn normalize_scales_by_compositor_over_surface() {
        let s = make();
        assert_eq!(s.normalize(100.0, 50.0), (200.0, 100.0));
    }

    #[test]
    fn pointer_motion_is_normalized() {
        let s = make();
        match s.pointer_motion(400.0, 300.0, 7) {
            WipMessage::PointerMotion(m) => {
                assert_eq!((m.x, m.y), (800.0, 600.0));
                assert_eq!(m.time_ms, 7);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn serialize_round_trips_through_proto() {
        let s = make();
        let msg = s.key_down(30, 0x4, 11);
        let mut bytes = BytesMut::from(serialize(&msg).unwrap().as_ref());
        let decoded = decode_wip(&mut bytes).unwrap().unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn stylus_preserves_pressure_and_tilt() {
        let s = make();
        let input = StylusInput {
            x: 10.0,
            y: 20.0,
            pressure: 0.5,
            tilt_x: 0.1,
            tilt_y: -0.2,
        };
        match s.stylus_motion(input, 3) {
            WipMessage::StylusMotion(m) => {
                assert_eq!((m.x, m.y), (20.0, 40.0));
                assert_eq!(m.pressure, 0.5);
                assert_eq!((m.tilt_x, m.tilt_y), (0.1, -0.2));
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
