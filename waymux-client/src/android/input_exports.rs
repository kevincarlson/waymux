//! JNI exports that forward Android input events to the bridge. Split out from
//! the lifecycle exports in `super` to keep each file focused.

use jni::JNIEnv;
use jni::objects::JClass;
use jni::sys::{jboolean, jfloat, jint, jlong};
use waymux_proto::{ButtonState, PointerAxis};

use super::{client, send_input};

/// Forwards an absolute pointer motion.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_nativeSendPointerMotion(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    x: jfloat,
    y: jfloat,
    time_ms: jint,
) {
    // SAFETY: `ptr` is a live handle per the JNI contract.
    if let Some(android) = unsafe { client(ptr) } {
        send_input(android, |input| input.pointer_motion(x, y, time_ms as u32));
    }
}

/// Forwards a pointer button press (`pressed != 0`) or release.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_nativeSendPointerButton(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    button: jint,
    pressed: jboolean,
    time_ms: jint,
) {
    let state = if pressed != 0 {
        ButtonState::Pressed
    } else {
        ButtonState::Released
    };
    // SAFETY: `ptr` is a live handle per the JNI contract.
    if let Some(android) = unsafe { client(ptr) } {
        send_input(android, |input| {
            input.pointer_button(button as u32, state, time_ms as u32)
        });
    }
}

/// Forwards a scroll event (`axis`: 0 = vertical, 1 = horizontal).
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_nativeSendScroll(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    axis: jint,
    value: jfloat,
    time_ms: jint,
) {
    let axis = if axis == 1 {
        PointerAxis::Horizontal
    } else {
        PointerAxis::Vertical
    };
    // SAFETY: `ptr` is a live handle per the JNI contract.
    if let Some(android) = unsafe { client(ptr) } {
        send_input(android, |input| {
            input.pointer_axis(axis, value, time_ms as u32)
        });
    }
}

/// Forwards a touch event (`action`: 0 = down, 1 = move, 2 = up).
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_nativeSendTouch(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    action: jint,
    id: jint,
    x: jfloat,
    y: jfloat,
    time_ms: jint,
) {
    // SAFETY: `ptr` is a live handle per the JNI contract.
    let Some(android) = (unsafe { client(ptr) }) else {
        return;
    };
    let (id, time_ms) = (id as u32, time_ms as u32);
    send_input(android, |input| match action {
        0 => input.touch_down(id, x, y, time_ms),
        2 => input.touch_up(id, time_ms),
        _ => input.touch_motion(id, x, y, time_ms),
    });
}

/// Forwards a key press (`pressed != 0`) or release.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_nativeSendKey(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    keycode: jint,
    modifiers: jint,
    pressed: jboolean,
    time_ms: jint,
) {
    // SAFETY: `ptr` is a live handle per the JNI contract.
    let Some(android) = (unsafe { client(ptr) }) else {
        return;
    };
    let (keycode, modifiers, time_ms) = (keycode as u32, modifiers as u32, time_ms as u32);
    send_input(android, |input| {
        if pressed != 0 {
            input.key_down(keycode, modifiers, time_ms)
        } else {
            input.key_up(keycode, modifiers, time_ms)
        }
    });
}
