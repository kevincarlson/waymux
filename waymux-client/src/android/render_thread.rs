//! Background render thread: owns the wgpu [`Renderer`] and presents the
//! client's latest frame each iteration, applying pending resizes.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ndk::native_window::NativeWindow;
use tokio::runtime::Handle;
use tracing::error;

use crate::renderer::Renderer;
use crate::state::ClientState;

/// Latest requested surface size plus a dirty flag.
#[derive(Default)]
struct ResizeState {
    width: AtomicU32,
    height: AtomicU32,
    dirty: AtomicBool,
}

/// Handle to a running render thread.
pub(super) struct RenderThread {
    stop: Arc<AtomicBool>,
    resize: Arc<ResizeState>,
    join: Option<JoinHandle<()>>,
}

impl RenderThread {
    /// Spawns a render thread that draws `client`'s frames onto `window`.
    pub(super) fn spawn(handle: Handle, client: Arc<ClientState>, window: NativeWindow) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let resize = Arc::new(ResizeState::default());
        let stop_thread = stop.clone();
        let resize_thread = resize.clone();
        let join = thread::Builder::new()
            .name("waymux-render".into())
            .spawn(move || render_loop(&handle, &client, window, &stop_thread, &resize_thread))
            .ok();
        Self { stop, resize, join }
    }

    /// Requests the renderer reconfigure to a new surface size.
    pub(super) fn request_resize(&self, width: u32, height: u32) {
        self.resize.width.store(width, Ordering::Relaxed);
        self.resize.height.store(height, Ordering::Relaxed);
        self.resize.dirty.store(true, Ordering::Release);
    }

    /// Signals the thread to stop and waits for it to finish.
    pub(super) fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn render_loop(
    handle: &Handle,
    client: &ClientState,
    window: NativeWindow,
    stop: &AtomicBool,
    resize: &ResizeState,
) {
    let mut renderer = match handle.block_on(Renderer::new(window)) {
        Ok(renderer) => renderer,
        Err(err) => {
            error!(error = %err, "failed to create renderer");
            return;
        }
    };

    while !stop.load(Ordering::Relaxed) {
        if resize.dirty.swap(false, Ordering::Acquire) {
            renderer.resize(
                resize.width.load(Ordering::Relaxed),
                resize.height.load(Ordering::Relaxed),
            );
        }
        match client.latest_frame() {
            Some(frame) => match renderer.render(&frame) {
                Ok(()) => {}
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    renderer.reconfigure();
                }
                Err(wgpu::SurfaceError::OutOfMemory) => break,
                Err(_) => {}
            },
            None => thread::sleep(Duration::from_millis(8)),
        }
    }
}
