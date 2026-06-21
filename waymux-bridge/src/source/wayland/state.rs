//! The Wayland dispatch state shared across all protocol callbacks, plus the
//! SCTK handler/delegate boilerplate for registry, output, and shm.

use bytes::Bytes;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::shm::raw::RawPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{delegate_output, delegate_registry, delegate_shm, registry_handlers};
use tracing::warn;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_shm;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use super::inject::Injector;
use crate::source::RawFrame;

/// Pixel parameters advertised by a screencopy `buffer` event.
#[derive(Debug, Clone, Copy)]
pub(super) struct BufferParams {
    pub(super) format: wl_shm::Format,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) stride: u32,
}

/// Capture progress for the in-flight screencopy frame.
#[derive(Default)]
pub(super) enum Capture {
    /// No capture in progress.
    #[default]
    Idle,
    /// A frame object exists and we are collecting its buffer parameters.
    Awaiting,
}

/// State threaded through every Wayland event callback on the capture thread.
pub(super) struct WaylandState {
    pub(super) registry_state: RegistryState,
    pub(super) output_state: OutputState,
    pub(super) shm: Shm,
    pub(super) qh: QueueHandle<WaylandState>,

    pub(super) screencopy: ZwlrScreencopyManagerV1,
    pub(super) output: Option<WlOutput>,
    pub(super) overlay_cursor: bool,

    pub(super) pool: RawPool,
    pub(super) capture: Capture,
    pub(super) params: Option<BufferParams>,
    pub(super) buffer: Option<WlBuffer>,
    pub(super) y_invert: bool,
    pub(super) ready: Option<RawFrame>,

    pub(super) injector: Injector,
}

impl WaylandState {
    /// Pulls the most recently completed frame, if any.
    pub(super) fn take_ready_frame(&mut self) -> Option<RawFrame> {
        self.ready.take()
    }

    /// Forwards a decoded input event to the injector.
    pub(super) fn inject_input(&mut self, msg: waymux_proto::WipMessage) {
        self.injector.inject(msg);
    }

    /// Reads the captured pixels out of the shm pool into a BGRA8 [`RawFrame`].
    pub(super) fn read_frame(&mut self, params: BufferParams) -> Option<RawFrame> {
        let row = params.width as usize * 4;
        let stride = params.stride as usize;
        let height = params.height as usize;
        let force_opaque = matches!(params.format, wl_shm::Format::Xrgb8888);

        let mmap = self.pool.mmap();
        let src = mmap.get(..stride * height)?;
        let mut out = bytes::BytesMut::with_capacity(row * height);
        for y in 0..height {
            // Honour the Y_INVERT flag by reading rows bottom-up when set.
            let sy = if self.y_invert { height - 1 - y } else { y };
            let line = &src[sy * stride..sy * stride + row];
            out.extend_from_slice(line);
            if force_opaque {
                let base = y * row;
                for px in (0..row).step_by(4) {
                    out[base + px + 3] = 0xff;
                }
            }
        }
        Some(RawFrame::new(
            params.width,
            params.height,
            params.width * 4,
            Bytes::from(out),
        ))
    }
}

impl ProvidesRegistryState for WaylandState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

impl ShmHandler for WaylandState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl OutputHandler for WaylandState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: WlOutput) {
        if self.output.as_ref() == Some(&output) {
            warn!("captured output was removed");
        }
    }
}

// The wl_buffer and wl_seat have no events we need; accept and ignore them.
impl Dispatch<WlBuffer, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WlBuffer,
        _: <WlBuffer as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: <WlSeat as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_registry!(WaylandState);
delegate_output!(WaylandState);
delegate_shm!(WaylandState);
