package app.appthere.waymux

import android.view.Surface

/**
 * JNI bindings to `libwaymux_client.so`. Method names mirror the
 * `Java_app_appthere_waymux_RustBridge_*` exports in the Rust crate.
 *
 * All methods are thin pass-throughs; the Rust side owns the connection,
 * decoding, rendering, and input serialization. The opaque `handle` returned
 * by [nativeInit] identifies the native client and must be passed to every
 * other call, then released exactly once with [nativeDestroy].
 */
object RustBridge {
    init {
        System.loadLibrary("waymux_client")
    }

    /** Connects to the bridge socket; returns a handle, or 0 on failure. */
    external fun nativeInit(socketPath: String): Long

    external fun nativeSurfaceCreated(handle: Long, surface: Surface)

    external fun nativeSurfaceChanged(handle: Long, width: Int, height: Int)

    external fun nativeSurfaceDestroyed(handle: Long)

    external fun nativeSendPointerMotion(handle: Long, x: Float, y: Float, timeMs: Int)

    external fun nativeSendPointerButton(handle: Long, button: Int, pressed: Boolean, timeMs: Int)

    /** Scroll: `axis` 0 = vertical, 1 = horizontal. */
    external fun nativeSendScroll(handle: Long, axis: Int, value: Float, timeMs: Int)

    /** Touch: `action` 0 = down, 1 = move, 2 = up. */
    external fun nativeSendTouch(
        handle: Long,
        action: Int,
        id: Int,
        x: Float,
        y: Float,
        timeMs: Int,
    )

    external fun nativeSendKey(
        handle: Long,
        keycode: Int,
        modifiers: Int,
        pressed: Boolean,
        timeMs: Int,
    )

    external fun nativeDestroy(handle: Long)
}
