//! `wlr-screencopy` capture: requesting frames and turning the compositor's
//! shm copy into a [`RawFrame`].

use tracing::{trace, warn};
use wayland_client::protocol::wl_shm;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{
    self, ZwlrScreencopyFrameV1,
};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use super::state::{BufferParams, Capture, WaylandState};

impl WaylandState {
    /// Requests capture of the output if no capture is already in flight.
    pub(super) fn begin_capture(&mut self) {
        if !matches!(self.capture, Capture::Idle) {
            return;
        }
        let Some(output) = self.output.clone() else {
            return;
        };
        let overlay = i32::from(self.overlay_cursor);
        let _frame = self
            .screencopy
            .capture_output(overlay, &output, &self.qh, ());
        self.capture = Capture::Awaiting;
        self.params = None;
        self.buffer = None;
        self.y_invert = false;
    }

    /// Allocates a shm buffer matching the advertised params and asks the
    /// compositor to copy into it. Idempotent within one capture.
    fn copy_into_pool(&mut self, frame: &ZwlrScreencopyFrameV1) {
        if self.buffer.is_some() {
            return;
        }
        let Some(params) = self.params else {
            warn!("screencopy buffer_done with no supported format");
            return;
        };
        let needed = params.stride as usize * params.height as usize;
        if self.pool.len() < needed
            && let Err(err) = self.pool.resize(needed)
        {
            warn!(error = %err, "failed to resize shm pool");
            return;
        }
        let buffer = self.pool.create_buffer::<WaylandState, ()>(
            0,
            params.width as i32,
            params.height as i32,
            params.stride as i32,
            params.format,
            (),
            &self.qh,
        );
        frame.copy(&buffer);
        self.buffer = Some(buffer);
    }

    fn finish_capture(&mut self, frame: &ZwlrScreencopyFrameV1) {
        if let Some(params) = self.params
            && let Some(decoded) = self.read_frame(params)
        {
            self.ready = Some(decoded);
        }
        if let Some(buffer) = self.buffer.take() {
            buffer.destroy();
        }
        frame.destroy();
        self.capture = Capture::Idle;
    }
}

/// Stores a `buffer` advertisement if it is a format we can forward as BGRA8.
fn record_format(state: &mut WaylandState, format: WEnum<wl_shm::Format>, w: u32, h: u32, s: u32) {
    if state.params.is_some() {
        return;
    }
    let Ok(format) = format.into_result() else {
        return;
    };
    if matches!(format, wl_shm::Format::Argb8888 | wl_shm::Format::Xrgb8888) {
        state.params = Some(BufferParams {
            format,
            width: w,
            height: h,
            stride: s,
        });
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ZwlrScreencopyManagerV1,
        _: <ZwlrScreencopyManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        frame: &ZwlrScreencopyFrameV1,
        event: <ZwlrScreencopyFrameV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwlr_screencopy_frame_v1::Event;
        match event {
            Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                record_format(state, format, width, height, stride);
                // v1/v2 have no buffer_done; copy as soon as a format is known.
                if frame.version() < 3 {
                    state.copy_into_pool(frame);
                }
            }
            Event::BufferDone => state.copy_into_pool(frame),
            Event::Flags { flags } => {
                if let Ok(flags) = flags.into_result() {
                    state.y_invert = flags.contains(zwlr_screencopy_frame_v1::Flags::YInvert);
                }
            }
            Event::Ready { .. } => state.finish_capture(frame),
            Event::Failed => {
                warn!("screencopy frame failed");
                if let Some(buffer) = state.buffer.take() {
                    buffer.destroy();
                }
                frame.destroy();
                state.capture = Capture::Idle;
            }
            Event::Damage { .. } | Event::LinuxDmabuf { .. } => trace!("ignored screencopy event"),
            _ => {}
        }
    }
}
