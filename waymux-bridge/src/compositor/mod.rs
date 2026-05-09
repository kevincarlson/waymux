// SPDX-License-Identifier: Apache-2.0
//! Wayland compositor client: globals, screencopy, and input injection.

pub mod input;
pub mod screencopy;

pub use input::{dispatch_wip_input, InputInjector};
pub use screencopy::ScreencopyState;

use bytes::Bytes;
use tokio::sync::mpsc;
use wayland_client::{
    globals::{registry_queue_init, GlobalList, GlobalListContents},
    protocol::{wl_output, wl_seat, wl_shm, wl_shm_pool, wl_registry},
    Connection, Dispatch, EventQueue, QueueHandle,
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_manager_v1::{self, ZwlrScreencopyManagerV1},
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::{self, ZwlrVirtualPointerManagerV1},
    zwlr_virtual_pointer_v1::{self, ZwlrVirtualPointerV1},
};

use crate::encoder::FrameEncoder;
use crate::error::BridgeError;

/// Lifecycle state of the compositor client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorClientState {
    /// Connecting and enumerating globals.
    Initializing,
    /// All required globals are bound and screencopy is active.
    Running,
    /// The compositor disconnected unexpectedly.
    CompositorLost,
    /// The bridge is shutting down gracefully.
    ShuttingDown,
    /// An unrecoverable error occurred.
    Error,
}

/// Central state object dispatched by the calloop Wayland event loop.
///
/// All `Dispatch<I, U>` implementations for this type live in this module or
/// in `compositor/screencopy.rs` and `compositor/input.rs`.
pub struct BridgeState {
    /// The screencopy manager global, available after the registry roundtrip.
    pub screencopy_manager: Option<ZwlrScreencopyManagerV1>,
    /// The virtual pointer manager global.
    pub vp_manager: Option<ZwlrVirtualPointerManagerV1>,
    /// A virtual pointer object for injecting pointer events.
    pub virtual_pointer: Option<ZwlrVirtualPointerV1>,
    /// The primary wl_seat (first one announced by the compositor).
    pub seat: Option<wl_seat::WlSeat>,
    /// The primary wl_output (first one announced).
    pub primary_output: Option<wl_output::WlOutput>,
    /// The wl_shm global for shared-memory buffer allocation.
    pub shm: Option<wl_shm::WlShm>,
    /// An active wl_shm_pool used for screencopy buffers. Optional.
    pub shm_pool: Option<wl_shm_pool::WlShmPool>,
    /// Screencopy-specific state (encoder, channel, dimensions).
    pub screencopy: ScreencopyState,
    /// Current lifecycle state.
    pub client_state: CompositorClientState,
    /// Output pixel width (updated from `wl_output::Event::Mode`).
    pub output_width: u32,
    /// Output pixel height (updated from `wl_output::Event::Mode`).
    pub output_height: u32,
}

impl BridgeState {
    /// Construct a new [`BridgeState`].
    pub fn new(frame_tx: mpsc::Sender<Bytes>, encoder: Box<dyn FrameEncoder>) -> Self {
        BridgeState {
            screencopy_manager: None,
            vp_manager: None,
            virtual_pointer: None,
            seat: None,
            primary_output: None,
            shm: None,
            shm_pool: None,
            screencopy: ScreencopyState::new(frame_tx, encoder),
            client_state: CompositorClientState::Initializing,
            output_width: 0,
            output_height: 0,
        }
    }
}

// ── Registry dispatch ────────────────────────────────────────────────────────

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for BridgeState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "zwlr_screencopy_manager_v1" => {
                    let mgr: ZwlrScreencopyManagerV1 =
                        registry.bind(name, version.min(3), qh, ());
                    state.screencopy_manager = Some(mgr);
                }
                "zwlr_virtual_pointer_manager_v1" => {
                    let vpm: ZwlrVirtualPointerManagerV1 =
                        registry.bind(name, version.min(2), qh, ());
                    state.vp_manager = Some(vpm);
                }
                "wl_output" => {
                    if state.primary_output.is_none() {
                        let out: wl_output::WlOutput =
                            registry.bind(name, version.min(4), qh, ());
                        state.primary_output = Some(out);
                    }
                }
                "wl_shm" => {
                    if state.shm.is_none() {
                        let shm: wl_shm::WlShm = registry.bind(name, 1, qh, ());
                        state.shm = Some(shm);
                    }
                }
                "wl_seat" => {
                    if state.seat.is_none() {
                        let seat: wl_seat::WlSeat =
                            registry.bind(name, version.min(8), qh, ());
                        state.seat = Some(seat);
                    }
                }
                _ => {}
            }
        }
    }
}

// ── No-event globals ─────────────────────────────────────────────────────────

impl Dispatch<ZwlrScreencopyManagerV1, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrScreencopyManagerV1,
        _event: zwlr_screencopy_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // ZwlrScreencopyManagerV1 has no events.
    }
}

impl Dispatch<ZwlrVirtualPointerManagerV1, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrVirtualPointerManagerV1,
        _event: zwlr_virtual_pointer_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // ZwlrVirtualPointerManagerV1 has no events.
    }
}

impl Dispatch<ZwlrVirtualPointerV1, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrVirtualPointerV1,
        _event: zwlr_virtual_pointer_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // ZwlrVirtualPointerV1 has no events.
    }
}

impl Dispatch<wl_shm::WlShm, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm::WlShm,
        _event: wl_shm::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // We receive format advertisements but don't need to act on them.
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm_pool::WlShmPool,
        _event: wl_shm_pool::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // WlShmPool has no events.
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for BridgeState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_seat::WlSeat,
        _event: wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // We receive capability advertisements; ignore them for now.
    }
}

impl Dispatch<wl_output::WlOutput, ()> for BridgeState {
    fn event(
        state: &mut Self,
        _proxy: &wl_output::WlOutput,
        event: wl_output::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Mode { flags: _, width, height, refresh: _ } = event
            && width > 0 && height > 0
        {
            state.output_width = width as u32;
            state.output_height = height as u32;
            tracing::debug!(width, height, "output mode updated");
        }
    }
}

// ── CompositorClient (handle returned to main) ───────────────────────────────

/// Opaque handle to the compositor connection and initial globals list.
pub struct CompositorClient {
    /// The Wayland connection.
    pub conn: Connection,
    /// Initial global list from the first roundtrip.
    pub globals: GlobalList,
}

/// Connect to the Wayland compositor and enumerate globals.
///
/// Returns a [`CompositorClient`] if the connection succeeds and all required
/// globals are present. Fails with [`BridgeError::MissingGlobal`] if any
/// required protocol is absent.
pub fn connect(
    _wayland_display: &str,
) -> Result<(CompositorClient, EventQueue<BridgeState>), BridgeError> {
    let conn = Connection::connect_to_env()?;
    let (globals, event_queue) =
        registry_queue_init::<BridgeState>(&conn).map_err(|e| {
            BridgeError::WaylandConnect(e.to_string())
        })?;

    // Verify required globals are present.
    let available: Vec<String> = globals
        .contents()
        .clone_list()
        .into_iter()
        .map(|g| g.interface)
        .collect();

    let required = ["zwlr_screencopy_manager_v1"];
    for iface in &required {
        if !available.iter().any(|a| a == iface) {
            return Err(BridgeError::MissingGlobal(iface));
        }
    }

    Ok((CompositorClient { conn, globals }, event_queue))
}
