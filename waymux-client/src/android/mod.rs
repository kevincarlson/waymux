//! JNI exports for the Android client. Function names follow the
//! `Java_app_appthere_waymux_RustBridge_<method>` convention and wrap an opaque
//! [`AndroidClient`] handle passed back and forth as a `jlong`.
//!
//! This module is the crate's only `unsafe` surface: the JNI FFI boundary plus
//! `ANativeWindow_fromSurface`. Every `unsafe` block documents its invariants.

mod input_exports;
mod render_thread;

use std::path::Path;
use std::sync::{Arc, Mutex, Once};

use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jint, jlong};
use ndk::native_window::NativeWindow;
use tracing::{error, warn};
use waymux_proto::WipMessage;

use crate::input::InputSerializer;
use crate::state::ClientState;
use render_thread::RenderThread;

/// Owns the runtime, bridge connection, input mapper, and render thread.
struct AndroidClient {
    runtime: tokio::runtime::Runtime,
    client: Arc<ClientState>,
    input: Mutex<InputSerializer>,
    render: Mutex<Option<RenderThread>>,
}

/// Recovers a shared reference to the client from its handle.
///
/// # Safety
/// `ptr` must be a non-zero handle returned by `nativeInit` and not yet passed
/// to `nativeDestroy`.
#[allow(unsafe_code)]
unsafe fn client<'a>(ptr: jlong) -> Option<&'a AndroidClient> {
    // SAFETY: per the contract above, `ptr` is a live `AndroidClient` pointer.
    unsafe { (ptr as *const AndroidClient).as_ref() }
}

/// Builds and queues an input message, refreshing the compositor size first.
fn send_input(android: &AndroidClient, build: impl FnOnce(&InputSerializer) -> WipMessage) {
    let msg = {
        let Ok(mut input) = android.input.lock() else {
            return;
        };
        if let Some(info) = android.client.display_info() {
            input.set_compositor_size(info.width, info.height);
        }
        build(&input)
    };
    if let Err(err) = android.client.try_send_input(msg) {
        warn!(error = %err, "input event dropped");
    }
}

fn init_logging() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;
        let layer = tracing_android::layer("waymux").ok();
        let _ = tracing_subscriber::registry().with(layer).try_init();
    });
}

/// Connects to the bridge and returns an opaque client handle (0 on failure).
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_nativeInit(
    mut env: JNIEnv,
    _class: JClass,
    socket_path: JString,
) -> jlong {
    init_logging();

    let path = match env.get_string(&socket_path) {
        Ok(value) => value.to_string_lossy().into_owned(),
        Err(err) => {
            error!(error = %err, "failed to read socket path");
            return 0;
        }
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            error!(error = %err, "failed to build runtime");
            return 0;
        }
    };

    let client = match runtime.block_on(ClientState::connect(Path::new(&path))) {
        Ok(client) => Arc::new(client),
        Err(err) => {
            error!(error = %err, "failed to connect to bridge");
            return 0;
        }
    };

    let android = AndroidClient {
        runtime,
        client,
        input: Mutex::new(InputSerializer::new()),
        render: Mutex::new(None),
    };
    Box::into_raw(Box::new(android)) as jlong
}

/// Attaches a `SurfaceView`'s `Surface` and starts rendering.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_nativeSurfaceCreated(
    env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    surface: JObject,
) {
    // SAFETY: `ptr` is a live handle per the JNI contract.
    let Some(android) = (unsafe { client(ptr) }) else {
        return;
    };

    // SAFETY: `env` and `surface` are valid for the duration of this JNI call;
    // `from_surface` acquires its own reference to the native window.
    #[allow(unsafe_code)]
    let window =
        unsafe { NativeWindow::from_surface(env.get_raw().cast(), surface.as_raw().cast()) };
    let Some(window) = window else {
        warn!("Surface had no native window");
        return;
    };

    if let Ok(mut input) = android.input.lock() {
        input.set_surface_size(window.width().max(0) as u32, window.height().max(0) as u32);
    }

    let thread = RenderThread::spawn(
        android.runtime.handle().clone(),
        android.client.clone(),
        window,
    );
    if let Ok(mut guard) = android.render.lock() {
        if let Some(old) = guard.take() {
            old.stop();
        }
        *guard = Some(thread);
    }
}

/// Notifies the renderer of a surface resize.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_nativeSurfaceChanged(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    width: jint,
    height: jint,
) {
    // SAFETY: `ptr` is a live handle per the JNI contract.
    let Some(android) = (unsafe { client(ptr) }) else {
        return;
    };
    let (w, h) = (width.max(0) as u32, height.max(0) as u32);
    if let Ok(mut input) = android.input.lock() {
        input.set_surface_size(w, h);
    }
    if let Ok(guard) = android.render.lock()
        && let Some(thread) = guard.as_ref()
    {
        thread.request_resize(w, h);
    }
}

/// Stops rendering when the surface is destroyed.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_nativeSurfaceDestroyed(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
) {
    // SAFETY: `ptr` is a live handle per the JNI contract.
    let Some(android) = (unsafe { client(ptr) }) else {
        return;
    };
    if let Ok(mut guard) = android.render.lock()
        && let Some(thread) = guard.take()
    {
        thread.stop();
    }
}

/// Releases the client handle. Must be called exactly once.
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_appthere_waymux_RustBridge_nativeDestroy(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
) {
    if ptr == 0 {
        return;
    }
    // SAFETY: `ptr` came from `Box::into_raw` in `nativeInit` and this is the
    // single matching `from_raw`, per the destroy-once contract.
    #[allow(unsafe_code)]
    let android = unsafe { Box::from_raw(ptr as *mut AndroidClient) };
    if let Ok(mut guard) = android.render.lock()
        && let Some(thread) = guard.take()
    {
        thread.stop();
    }
}
