//! Wayland `wlr-screencopy` frame source and `wlr-virtual-pointer` injection.
//!
//! [`WaylandScreencopySource`] connects to the compositor named by
//! `$WAYLAND_DISPLAY`, captures the primary output, and emits [`RawFrame`]s. It
//! also exposes an [input sender](WaylandScreencopySource::input_sender) the
//! server uses to forward WIP events for injection.
//!
//! Capture and injection run on one dedicated thread driven by `calloop`, which
//! polls the Wayland socket and the input channel together. The thread is the
//! body of the [`FrameSource::run`] future (via `spawn_blocking`).

mod inject;
mod screencopy;
mod state;

use std::time::Duration;

use calloop::EventLoop;
use calloop::channel::{Channel, Event as ChannelEvent, Sender, channel};
use calloop::timer::{TimeoutAction, Timer};
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::output::OutputState;
use smithay_client_toolkit::registry::RegistryState;
use smithay_client_toolkit::shm::Shm;
use smithay_client_toolkit::shm::raw::RawPool;
use tokio::sync::mpsc;
use wayland_client::Connection;
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1;
use waymux_proto::{DisplayInfoMsg, WipMessage};

use self::inject::Injector;
use self::state::{Capture, WaylandState};
use crate::error::BridgeError;
use crate::source::{FrameSource, RawFrame};

/// Initial shm pool size (1080p BGRA8); resized to the output on connect.
const INITIAL_POOL: usize = 1920 * 1080 * 4;

/// A [`FrameSource`] that captures a Wayland compositor via `wlr-screencopy`.
pub struct WaylandScreencopySource {
    conn: Connection,
    event_queue: wayland_client::EventQueue<WaylandState>,
    state: WaylandState,
    display_info: DisplayInfoMsg,
    input_tx: Sender<WipMessage>,
    input_rx: Channel<WipMessage>,
    max_fps: u32,
}

fn src_err(context: &str, err: impl std::fmt::Display) -> BridgeError {
    BridgeError::Source(format!("{context}: {err}"))
}

impl WaylandScreencopySource {
    /// Connects to the compositor, binds the required globals, selects the
    /// primary output, and prepares capture + injection.
    ///
    /// # Errors
    /// Returns [`BridgeError::Source`] if the connection fails or a required
    /// global (`wlr-screencopy`, `wlr-virtual-pointer`, `wl_shm`, an output) is
    /// missing.
    pub fn new(overlay_cursor: bool, max_fps: u32) -> Result<Self, BridgeError> {
        let conn = Connection::connect_to_env().map_err(|e| src_err("wayland connect", e))?;
        let (globals, mut event_queue) =
            registry_queue_init::<WaylandState>(&conn).map_err(|e| src_err("registry init", e))?;
        let qh = event_queue.handle();

        let registry_state = RegistryState::new(&globals);
        let output_state = OutputState::new(&globals, &qh);
        let shm = Shm::bind(&globals, &qh).map_err(|e| src_err("wl_shm", e))?;

        let screencopy: ZwlrScreencopyManagerV1 = globals
            .bind(&qh, 1..=3, ())
            .map_err(|e| src_err("wlr-screencopy not available", e))?;
        let vpointer: ZwlrVirtualPointerManagerV1 = globals
            .bind(&qh, 1..=2, ())
            .map_err(|e| src_err("wlr-virtual-pointer not available", e))?;
        let seat: WlSeat = globals
            .bind(&qh, 1..=8, ())
            .map_err(|e| src_err("wl_seat", e))?;

        let pointer = vpointer.create_virtual_pointer(Some(&seat), &qh, ());
        let pool = RawPool::new(INITIAL_POOL, &shm).map_err(|e| src_err("shm pool", e))?;

        let mut state = WaylandState {
            registry_state,
            output_state,
            shm,
            qh: qh.clone(),
            screencopy,
            output: None,
            overlay_cursor,
            pool,
            capture: Capture::Idle,
            params: None,
            buffer: None,
            y_invert: false,
            ready: None,
            injector: Injector::new(pointer, 1, 1),
        };

        // Roundtrip so OutputState learns about the outputs and their modes.
        event_queue
            .roundtrip(&mut state)
            .map_err(|e| src_err("output roundtrip", e))?;

        let output = state
            .output_state
            .outputs()
            .next()
            .ok_or_else(|| BridgeError::Source("no Wayland output found".into()))?;
        let info = state
            .output_state
            .info(&output)
            .ok_or_else(|| BridgeError::Source("output has no info".into()))?;

        let (width, height) = output_dimensions(&info);
        let refresh_hz = output_refresh(&info);
        state.output = Some(output);
        state.injector.set_extent(width, height);
        state
            .pool
            .resize((width as usize * height as usize * 4).max(INITIAL_POOL))
            .map_err(|e| src_err("resize pool", e))?;

        let display_info = DisplayInfoMsg {
            width,
            height,
            scale_factor: info.scale_factor as f32,
            refresh_hz,
        };

        let (input_tx, input_rx) = channel();
        Ok(Self {
            conn,
            event_queue,
            state,
            display_info,
            input_tx,
            input_rx,
            max_fps: max_fps.max(1),
        })
    }

    /// Returns a sender the server uses to forward WIP input for injection.
    #[must_use]
    pub fn input_sender(&self) -> Sender<WipMessage> {
        self.input_tx.clone()
    }

    /// Drives capture + injection on the calloop event loop until the frame
    /// channel closes.
    fn capture_loop(self, frames_tx: mpsc::Sender<RawFrame>) -> Result<(), BridgeError> {
        let WaylandScreencopySource {
            conn,
            event_queue,
            mut state,
            input_rx,
            max_fps,
            ..
        } = self;

        let mut event_loop: EventLoop<WaylandState> =
            EventLoop::try_new().map_err(|e| src_err("event loop", e))?;
        let handle = event_loop.handle();

        WaylandSource::new(conn.clone(), event_queue)
            .insert(handle.clone())
            .map_err(|e| src_err("wayland source", e))?;

        handle
            .insert_source(input_rx, |event, _, state| {
                if let ChannelEvent::Msg(msg) = event {
                    state.inject_input(msg);
                }
            })
            .map_err(|e| src_err("input source", e))?;

        let period = Duration::from_secs_f64(1.0 / f64::from(max_fps));
        handle
            .insert_source(Timer::from_duration(period), move |_, _, state| {
                state.begin_capture();
                TimeoutAction::ToDuration(period)
            })
            .map_err(|e| src_err("timer", e))?;

        state.begin_capture();
        loop {
            event_loop
                .dispatch(Some(period), &mut state)
                .map_err(|e| src_err("dispatch", e))?;
            let _ = conn.flush();
            if let Some(frame) = state.take_ready_frame()
                && frames_tx.blocking_send(frame).is_err()
            {
                return Ok(());
            }
        }
    }
}

impl FrameSource for WaylandScreencopySource {
    fn display_info(&self) -> DisplayInfoMsg {
        self.display_info
    }

    async fn run(self, tx: mpsc::Sender<RawFrame>) -> Result<(), BridgeError> {
        tokio::task::spawn_blocking(move || self.capture_loop(tx))
            .await
            .map_err(|e| src_err("capture thread", e))?
    }
}

fn output_dimensions(info: &smithay_client_toolkit::output::OutputInfo) -> (u32, u32) {
    if let Some((w, h)) = info.logical_size {
        return (w.max(1) as u32, h.max(1) as u32);
    }
    info.modes
        .iter()
        .find(|mode| mode.current)
        .map(|mode| {
            (
                mode.dimensions.0.max(1) as u32,
                mode.dimensions.1.max(1) as u32,
            )
        })
        .unwrap_or((1920, 1080))
}

fn output_refresh(info: &smithay_client_toolkit::output::OutputInfo) -> f32 {
    info.modes
        .iter()
        .find(|mode| mode.current)
        .map(|mode| mode.refresh_rate as f32 / 1000.0)
        .unwrap_or(60.0)
}
