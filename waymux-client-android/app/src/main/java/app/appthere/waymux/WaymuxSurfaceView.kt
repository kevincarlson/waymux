package app.appthere.waymux

import android.annotation.SuppressLint
import android.content.Context
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView

/**
 * Full-screen [SurfaceView] whose lifecycle and input are wired to the native
 * client identified by [handle]. The Rust render thread draws directly into the
 * surface's `ANativeWindow`.
 */
@SuppressLint("ViewConstructor")
class WaymuxSurfaceView(context: Context, private val handle: Long) :
    SurfaceView(context), SurfaceHolder.Callback {

    init {
        holder.addCallback(this)
        isFocusable = true
        isFocusableInTouchMode = true
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        RustBridge.nativeSurfaceCreated(handle, holder.surface)
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        RustBridge.nativeSurfaceChanged(handle, width, height)
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        RustBridge.nativeSurfaceDestroyed(handle)
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        return InputForwarder.forwardTouch(handle, event) || super.onTouchEvent(event)
    }

    override fun onGenericMotionEvent(event: MotionEvent): Boolean {
        return InputForwarder.forwardGenericMotion(handle, event) || super.onGenericMotionEvent(event)
    }
}
