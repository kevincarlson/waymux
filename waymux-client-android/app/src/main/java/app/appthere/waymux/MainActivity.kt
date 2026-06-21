package app.appthere.waymux

import android.app.Activity
import android.os.Bundle
import android.view.KeyEvent
import android.view.WindowManager
import android.widget.Toast

/**
 * Thin Activity shell: connects to the bridge, hosts a [WaymuxSurfaceView], and
 * forwards hardware key events. All rendering and input handling happen in the
 * Rust library; this class only manages the Android lifecycle.
 *
 * The bridge socket path defaults to Termux's shared tmp path and can be
 * overridden with an intent extra, e.g.:
 *
 * ```
 * adb shell am start -n app.appthere.waymux/.MainActivity \
 *     --es waymux_socket /data/data/com.termux/files/usr/tmp/waymux.sock
 * ```
 */
class MainActivity : Activity() {

    private var handle: Long = 0L

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)

        val socketPath = intent.getStringExtra(EXTRA_SOCKET) ?: DEFAULT_SOCKET
        handle = RustBridge.nativeInit(socketPath)
        if (handle == 0L) {
            Toast.makeText(this, "Failed to connect to bridge at $socketPath", Toast.LENGTH_LONG)
                .show()
            finish()
            return
        }

        setContentView(WaymuxSurfaceView(this, handle))
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (handle != 0L && InputForwarder.forwardKey(handle, event)) {
            return true
        }
        return super.dispatchKeyEvent(event)
    }

    override fun onDestroy() {
        super.onDestroy()
        if (handle != 0L) {
            RustBridge.nativeDestroy(handle)
            handle = 0L
        }
    }

    companion object {
        /** Intent extra to override the bridge socket path. */
        const val EXTRA_SOCKET = "waymux_socket"

        /** Default Termux shared-tmp socket path. */
        const val DEFAULT_SOCKET = "/data/data/com.termux/files/usr/tmp/waymux.sock"
    }
}
