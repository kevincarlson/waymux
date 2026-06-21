//! Input injection: maps WIP events onto a `zwlr-virtual-pointer` device.
//!
//! Pointer, scroll, and touch (emulated as a left button) are injected. Key and
//! stylus events are decoded but not yet injected — keyboard injection needs a
//! virtual-keyboard keymap and an Android→evdev keycode map (a follow-up).

use tracing::trace;
use wayland_client::protocol::wl_pointer::{Axis, ButtonState as WlButtonState};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1;
use waymux_proto::{ButtonState, PointerAxis, WipMessage};

use super::state::WaylandState;

/// Linux evdev code for the primary (left) mouse button.
const BTN_LEFT: u32 = 0x110;

/// Injects WIP input onto a virtual pointer scaled to the output extent.
pub(super) struct Injector {
    pointer: ZwlrVirtualPointerV1,
    extent_w: u32,
    extent_h: u32,
}

impl Injector {
    pub(super) fn new(pointer: ZwlrVirtualPointerV1, extent_w: u32, extent_h: u32) -> Self {
        Self {
            pointer,
            extent_w,
            extent_h,
        }
    }

    /// Updates the motion extent to match the captured output size.
    pub(super) fn set_extent(&mut self, width: u32, height: u32) {
        self.extent_w = width.max(1);
        self.extent_h = height.max(1);
    }

    /// Applies one decoded input event.
    pub(super) fn inject(&mut self, msg: WipMessage) {
        match msg {
            WipMessage::PointerMotion(m) => self.motion(m.x, m.y, m.time_ms),
            WipMessage::PointerButton(m) => self.button(m.button, m.state, m.time_ms),
            WipMessage::PointerAxis(m) => self.axis(m.axis, m.value, m.time_ms),
            WipMessage::TouchDown(m) => {
                self.motion(m.x, m.y, m.time_ms);
                self.button(BTN_LEFT, ButtonState::Pressed, m.time_ms);
            }
            WipMessage::TouchMotion(m) => self.motion(m.x, m.y, m.time_ms),
            WipMessage::TouchUp { time_ms, .. } => {
                self.button(BTN_LEFT, ButtonState::Released, time_ms);
            }
            WipMessage::KeyDown(_) | WipMessage::KeyUp(_) => {
                trace!("keyboard injection not yet implemented");
            }
            WipMessage::StylusDown(_)
            | WipMessage::StylusMotion(_)
            | WipMessage::StylusUp { .. } => {
                trace!("stylus injection not yet implemented");
            }
            WipMessage::Pong { .. } => {}
        }
    }

    fn motion(&self, x: f32, y: f32, time_ms: u32) {
        self.pointer.motion_absolute(
            time_ms,
            clamp(x, self.extent_w),
            clamp(y, self.extent_h),
            self.extent_w,
            self.extent_h,
        );
        self.pointer.frame();
    }

    fn button(&self, button: u32, state: ButtonState, time_ms: u32) {
        let state = match state {
            ButtonState::Pressed => WlButtonState::Pressed,
            ButtonState::Released => WlButtonState::Released,
        };
        self.pointer.button(time_ms, button, state);
        self.pointer.frame();
    }

    fn axis(&self, axis: PointerAxis, value: f32, time_ms: u32) {
        let axis = match axis {
            PointerAxis::Vertical => Axis::VerticalScroll,
            PointerAxis::Horizontal => Axis::HorizontalScroll,
        };
        self.pointer.axis(time_ms, axis, f64::from(value));
        self.pointer.frame();
    }
}

fn clamp(value: f32, max: u32) -> u32 {
    value.clamp(0.0, max as f32) as u32
}

impl Dispatch<ZwlrVirtualPointerManagerV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ZwlrVirtualPointerManagerV1,
        _: <ZwlrVirtualPointerManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrVirtualPointerV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ZwlrVirtualPointerV1,
        _: <ZwlrVirtualPointerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
