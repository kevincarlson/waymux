// SPDX-License-Identifier: Apache-2.0
//! Input injection abstraction and WIP→Wayland dispatch.

use waymux_proto::{PointerAxis, WipMessage};
use crate::error::BridgeError;

/// Injects input events into a Wayland compositor session.
///
/// Implementations must forward each method to the appropriate Wayland
/// virtual input object. The [`MockInjector`] implementation is provided
/// for unit-testing the dispatch logic without a real compositor.
pub trait InputInjector {
    /// Inject a pointer motion to an absolute position in compositor pixels.
    fn pointer_motion(&mut self, x: f64, y: f64, time_ms: u32) -> Result<(), BridgeError>;

    /// Inject a pointer button press or release.
    ///
    /// `button` is a Linux input-event button code (e.g. `BTN_LEFT = 0x110`).
    fn pointer_button(
        &mut self,
        button: u32,
        pressed: bool,
        time_ms: u32,
    ) -> Result<(), BridgeError>;

    /// Inject a scroll-wheel or touchpad scroll axis event.
    fn pointer_axis(
        &mut self,
        axis: PointerAxis,
        value: f64,
        time_ms: u32,
    ) -> Result<(), BridgeError>;

    /// Inject a touch contact start (finger down).
    fn touch_down(
        &mut self,
        id: u32,
        x: f64,
        y: f64,
        time_ms: u32,
    ) -> Result<(), BridgeError>;

    /// Inject a touch contact move.
    fn touch_motion(
        &mut self,
        id: u32,
        x: f64,
        y: f64,
        time_ms: u32,
    ) -> Result<(), BridgeError>;

    /// Inject a touch contact lift (finger up).
    fn touch_up(&mut self, id: u32, time_ms: u32) -> Result<(), BridgeError>;

    /// Inject a key press or release event.
    ///
    /// `keycode` is a Linux key code; `pressed` is `true` for key-down.
    fn key_event(
        &mut self,
        keycode: u32,
        pressed: bool,
        time_ms: u32,
    ) -> Result<(), BridgeError>;
}

/// Dispatch a [`WipMessage`] to the appropriate [`InputInjector`] method.
///
/// This function is the single mapping point between the WIP protocol and
/// the Wayland input injection abstraction, making it straightforward to
/// unit-test with [`MockInjector`].
pub fn dispatch_wip_input<I: InputInjector>(
    msg: &WipMessage,
    injector: &mut I,
) -> Result<(), BridgeError> {
    match msg {
        WipMessage::PointerMotion(m) => {
            injector.pointer_motion(m.x as f64, m.y as f64, m.time_ms)
        }
        WipMessage::PointerButton(m) => {
            let pressed = m.state == waymux_proto::ButtonState::Pressed;
            injector.pointer_button(m.button, pressed, m.time_ms)
        }
        WipMessage::PointerAxis(m) => {
            injector.pointer_axis(m.axis, m.value as f64, m.time_ms)
        }
        WipMessage::TouchDown(m) => {
            injector.touch_down(m.id, m.x as f64, m.y as f64, m.time_ms)
        }
        WipMessage::TouchMotion(m) => {
            injector.touch_motion(m.id, m.x as f64, m.y as f64, m.time_ms)
        }
        WipMessage::TouchUp { id, time_ms } => {
            injector.touch_up(*id, *time_ms)
        }
        WipMessage::StylusDown(m) => {
            // Map stylus to pointer motion + button press
            injector.pointer_motion(m.x as f64, m.y as f64, m.time_ms)?;
            injector.pointer_button(0x110 /* BTN_LEFT */, true, m.time_ms)
        }
        WipMessage::StylusMotion(m) => {
            injector.pointer_motion(m.x as f64, m.y as f64, m.time_ms)
        }
        WipMessage::StylusUp { time_ms } => {
            injector.pointer_button(0x110 /* BTN_LEFT */, false, *time_ms)
        }
        WipMessage::KeyDown(m) => {
            injector.key_event(m.keycode, true, m.time_ms)
        }
        WipMessage::KeyUp(m) => {
            injector.key_event(m.keycode, false, m.time_ms)
        }
        WipMessage::Pong { .. } => {
            // Keepalive — no input injection needed.
            Ok(())
        }
    }
}

/// A test-helper [`InputInjector`] that records all injected events.
///
/// Use [`MockInjector::events`] to assert the sequence of events after
/// calling [`dispatch_wip_input`].
pub struct MockInjector {
    /// All events recorded in the order they were injected.
    pub events: Vec<MockEvent>,
}

impl MockInjector {
    /// Create an empty [`MockInjector`].
    pub fn new() -> Self {
        MockInjector { events: Vec::new() }
    }
}

impl Default for MockInjector {
    fn default() -> Self {
        Self::new()
    }
}

/// A single input event recorded by [`MockInjector`].
#[derive(Debug, PartialEq)]
pub enum MockEvent {
    /// Pointer moved.
    PointerMotion {
        /// Horizontal position in compositor pixels.
        x: f64,
        /// Vertical position in compositor pixels.
        y: f64,
        /// Compositor time in milliseconds.
        time_ms: u32,
    },
    /// Pointer button pressed or released.
    PointerButton {
        /// Linux input-event button code.
        button: u32,
        /// `true` = pressed, `false` = released.
        pressed: bool,
        /// Compositor time in milliseconds.
        time_ms: u32,
    },
    /// Scroll axis event.
    PointerAxis {
        /// Which axis scrolled.
        axis: PointerAxis,
        /// Scroll distance in logical pixels.
        value: f64,
        /// Compositor time in milliseconds.
        time_ms: u32,
    },
    /// Touch contact started.
    TouchDown {
        /// Touch contact identifier.
        id: u32,
        /// Horizontal position in compositor pixels.
        x: f64,
        /// Vertical position in compositor pixels.
        y: f64,
        /// Compositor time in milliseconds.
        time_ms: u32,
    },
    /// Touch contact moved.
    TouchMotion {
        /// Touch contact identifier.
        id: u32,
        /// Horizontal position in compositor pixels.
        x: f64,
        /// Vertical position in compositor pixels.
        y: f64,
        /// Compositor time in milliseconds.
        time_ms: u32,
    },
    /// Touch contact ended.
    TouchUp {
        /// Touch contact identifier.
        id: u32,
        /// Compositor time in milliseconds.
        time_ms: u32,
    },
    /// Key pressed or released.
    KeyEvent {
        /// Linux key code.
        keycode: u32,
        /// `true` = pressed, `false` = released.
        pressed: bool,
        /// Compositor time in milliseconds.
        time_ms: u32,
    },
}

impl InputInjector for MockInjector {
    fn pointer_motion(&mut self, x: f64, y: f64, time_ms: u32) -> Result<(), BridgeError> {
        self.events.push(MockEvent::PointerMotion { x, y, time_ms });
        Ok(())
    }

    fn pointer_button(
        &mut self,
        button: u32,
        pressed: bool,
        time_ms: u32,
    ) -> Result<(), BridgeError> {
        self.events.push(MockEvent::PointerButton { button, pressed, time_ms });
        Ok(())
    }

    fn pointer_axis(
        &mut self,
        axis: PointerAxis,
        value: f64,
        time_ms: u32,
    ) -> Result<(), BridgeError> {
        self.events.push(MockEvent::PointerAxis { axis, value, time_ms });
        Ok(())
    }

    fn touch_down(
        &mut self,
        id: u32,
        x: f64,
        y: f64,
        time_ms: u32,
    ) -> Result<(), BridgeError> {
        self.events.push(MockEvent::TouchDown { id, x, y, time_ms });
        Ok(())
    }

    fn touch_motion(
        &mut self,
        id: u32,
        x: f64,
        y: f64,
        time_ms: u32,
    ) -> Result<(), BridgeError> {
        self.events.push(MockEvent::TouchMotion { id, x, y, time_ms });
        Ok(())
    }

    fn touch_up(&mut self, id: u32, time_ms: u32) -> Result<(), BridgeError> {
        self.events.push(MockEvent::TouchUp { id, time_ms });
        Ok(())
    }

    fn key_event(
        &mut self,
        keycode: u32,
        pressed: bool,
        time_ms: u32,
    ) -> Result<(), BridgeError> {
        self.events.push(MockEvent::KeyEvent { keycode, pressed, time_ms });
        Ok(())
    }
}
