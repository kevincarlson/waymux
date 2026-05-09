// SPDX-License-Identifier: Apache-2.0
//! Integration tests for WIP → InputInjector dispatch.

use waymux_proto::{
    ButtonState, KeyMsg, PointerAxis, PointerAxisMsg, PointerButtonMsg, PointerMotionMsg,
    StylusMsg, TouchPointMsg, WipMessage,
};
use waymux_bridge::compositor::input::{
    dispatch_wip_input, MockEvent, MockInjector,
};

#[test]
fn pointer_motion_dispatches_correctly() {
    let mut inj = MockInjector::new();
    let msg = WipMessage::PointerMotion(PointerMotionMsg { x: 42.5, y: 100.0, time_ms: 999 });
    dispatch_wip_input(&msg, &mut inj).expect("dispatch should succeed");

    assert_eq!(inj.events.len(), 1);
    assert_eq!(
        inj.events[0],
        MockEvent::PointerMotion { x: 42.5, y: 100.0, time_ms: 999 }
    );
}

#[test]
fn key_down_dispatches_correctly() {
    let mut inj = MockInjector::new();
    let msg = WipMessage::KeyDown(KeyMsg { keycode: 65, modifiers: 0, time_ms: 500 });
    dispatch_wip_input(&msg, &mut inj).expect("dispatch should succeed");

    assert_eq!(inj.events.len(), 1);
    assert_eq!(
        inj.events[0],
        MockEvent::KeyEvent { keycode: 65, pressed: true, time_ms: 500 }
    );
}

#[test]
fn key_up_dispatches_correctly() {
    let mut inj = MockInjector::new();
    let msg = WipMessage::KeyUp(KeyMsg { keycode: 65, modifiers: 0, time_ms: 600 });
    dispatch_wip_input(&msg, &mut inj).expect("dispatch should succeed");

    assert_eq!(inj.events.len(), 1);
    assert_eq!(
        inj.events[0],
        MockEvent::KeyEvent { keycode: 65, pressed: false, time_ms: 600 }
    );
}

#[test]
fn touch_down_dispatches_correctly() {
    let mut inj = MockInjector::new();
    let msg = WipMessage::TouchDown(TouchPointMsg { id: 3, x: 200.0, y: 300.0, time_ms: 1000 });
    dispatch_wip_input(&msg, &mut inj).expect("dispatch should succeed");

    assert_eq!(inj.events.len(), 1);
    assert_eq!(
        inj.events[0],
        MockEvent::TouchDown { id: 3, x: 200.0, y: 300.0, time_ms: 1000 }
    );
}

#[test]
fn pointer_button_dispatches_correctly() {
    let mut inj = MockInjector::new();
    let msg = WipMessage::PointerButton(PointerButtonMsg {
        button: 0x110,
        state: ButtonState::Pressed,
        time_ms: 123,
    });
    dispatch_wip_input(&msg, &mut inj).expect("dispatch should succeed");

    assert_eq!(inj.events.len(), 1);
    assert_eq!(
        inj.events[0],
        MockEvent::PointerButton { button: 0x110, pressed: true, time_ms: 123 }
    );
}

#[test]
fn pointer_axis_dispatches_correctly() {
    let mut inj = MockInjector::new();
    let msg = WipMessage::PointerAxis(PointerAxisMsg {
        axis: PointerAxis::Vertical,
        value: -3.0,
        time_ms: 200,
    });
    dispatch_wip_input(&msg, &mut inj).expect("dispatch should succeed");

    assert_eq!(inj.events.len(), 1);
    assert_eq!(
        inj.events[0],
        MockEvent::PointerAxis { axis: PointerAxis::Vertical, value: -3.0, time_ms: 200 }
    );
}

#[test]
fn stylus_down_emulates_pointer_motion_and_button() {
    let mut inj = MockInjector::new();
    let msg = WipMessage::StylusDown(StylusMsg {
        x: 50.0,
        y: 75.0,
        pressure: 0.8,
        tilt_x: 0.0,
        tilt_y: 0.0,
        time_ms: 300,
    });
    dispatch_wip_input(&msg, &mut inj).expect("dispatch should succeed");

    // StylusDown maps to PointerMotion + PointerButton(BTN_LEFT, pressed=true).
    assert_eq!(inj.events.len(), 2);
    assert_eq!(
        inj.events[0],
        MockEvent::PointerMotion { x: 50.0, y: 75.0, time_ms: 300 }
    );
    assert_eq!(
        inj.events[1],
        MockEvent::PointerButton { button: 0x110, pressed: true, time_ms: 300 }
    );
}

#[test]
fn pong_produces_no_injection() {
    let mut inj = MockInjector::new();
    let msg = WipMessage::Pong { sequence: 42 };
    dispatch_wip_input(&msg, &mut inj).expect("dispatch should succeed");
    assert!(inj.events.is_empty(), "Pong must not inject any input events");
}
