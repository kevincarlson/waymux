package app.appthere.waymux

import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent

/**
 * Translates Android [MotionEvent]/[KeyEvent]s into [RustBridge] calls. The
 * Rust side normalizes surface coordinates into compositor space, so raw event
 * coordinates are forwarded as-is.
 */
object InputForwarder {

    /** Forwards a touchscreen event. Returns true if consumed. */
    fun forwardTouch(handle: Long, event: MotionEvent): Boolean {
        val time = event.eventTime.toInt()
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                val i = event.actionIndex
                RustBridge.nativeSendTouch(
                    handle, ACTION_DOWN, event.getPointerId(i), event.getX(i), event.getY(i), time,
                )
            }

            MotionEvent.ACTION_MOVE -> {
                for (i in 0 until event.pointerCount) {
                    RustBridge.nativeSendTouch(
                        handle, ACTION_MOVE, event.getPointerId(i), event.getX(i), event.getY(i), time,
                    )
                }
            }

            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP, MotionEvent.ACTION_CANCEL -> {
                val i = event.actionIndex
                RustBridge.nativeSendTouch(
                    handle, ACTION_UP, event.getPointerId(i), event.getX(i), event.getY(i), time,
                )
            }

            else -> return false
        }
        return true
    }

    /** Forwards mouse/trackpad motion and scroll. Returns true if consumed. */
    fun forwardGenericMotion(handle: Long, event: MotionEvent): Boolean {
        if (event.source and InputDevice.SOURCE_CLASS_POINTER == 0) return false
        val time = event.eventTime.toInt()
        when (event.actionMasked) {
            MotionEvent.ACTION_HOVER_MOVE, MotionEvent.ACTION_MOVE ->
                RustBridge.nativeSendPointerMotion(handle, event.x, event.y, time)

            MotionEvent.ACTION_SCROLL -> {
                val vertical = event.getAxisValue(MotionEvent.AXIS_VSCROLL)
                if (vertical != 0f) RustBridge.nativeSendScroll(handle, AXIS_VERTICAL, vertical, time)
                val horizontal = event.getAxisValue(MotionEvent.AXIS_HSCROLL)
                if (horizontal != 0f) RustBridge.nativeSendScroll(handle, AXIS_HORIZONTAL, horizontal, time)
            }

            else -> return false
        }
        return true
    }

    /** Forwards a hardware key event. Returns true if consumed. */
    fun forwardKey(handle: Long, event: KeyEvent): Boolean {
        val pressed = when (event.action) {
            KeyEvent.ACTION_DOWN -> true
            KeyEvent.ACTION_UP -> false
            else -> return false
        }
        RustBridge.nativeSendKey(
            handle, event.keyCode, event.metaState, pressed, event.eventTime.toInt(),
        )
        return true
    }

    private const val ACTION_DOWN = 0
    private const val ACTION_MOVE = 1
    private const val ACTION_UP = 2
    private const val AXIS_VERTICAL = 0
    private const val AXIS_HORIZONTAL = 1
}
