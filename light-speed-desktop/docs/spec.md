# Light Speed Desktop — Package Specification

**Crate:** `light-speed-desktop`  
**Type:** Binary (`bin`)  
**Version:** 0.1.0  
**License:** Apache-2.0  
**Targets:** `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-gnu`

---

## Purpose

Light Speed Desktop (LSD) is a lightweight, responsive Wayland compositor and desktop environment designed to run comfortably on mobile-class hardware. It uses a **hybrid stacking/tiling window manager** that adapts its layout strategy based on display geometry, and it exposes the Wayland protocol extensions required by Waymux Bridge for frame capture.

LSD is the first-class compositor target for the Waymux ecosystem, but it is designed to be fully self-contained and usable as a general-purpose lightweight desktop.

---

## Design Principles

- **Responsive first:** Layout adapts automatically across form factors (phone portrait → tablet landscape → 1080p desktop) using breakpoint-driven rules, not user configuration.
- **Lightweight:** Minimal memory and CPU overhead. Targets ARM SBCs and mid-range phones running Termux.
- **Composability:** Smithay handles all Wayland protocol machinery; LSD provides the policy layer on top.
- **Avoid NIH:** Use Smithay, Iced (or GTK4), and existing crates rather than building custom solutions.

---

## Dependencies

```toml
[dependencies]
smithay                = { version = "0.7", features = ["wayland_frontend", "backend_drm",
                                                         "backend_winit", "backend_udev",
                                                         "xwayland", "renderer_gl",
                                                         "renderer_pixman"] }
calloop                = "0.13"
calloop-wayland-source = "0.3"
wayland-server         = "0.31"
wayland-protocols      = "0.31"
wayland-protocols-wlr  = "0.2"
iced                   = { version = "0.12", optional = true }     # preferred DE chrome
gtk4                   = { version = "0.8", optional = true }      # fallback DE chrome
tokio                  = { version = "1", features = ["rt", "sync"] }
thiserror              = "1"
tracing                = "0.1"
tracing-subscriber     = { version = "0.3", features = ["env-filter"] }
serde                  = { version = "1", features = ["derive"] }
toml                   = "0.8"
clap                   = { version = "4", features = ["derive", "env"] }
xkbcommon              = "0.7"

[features]
default = ["chrome-iced"]
chrome-iced = ["dep:iced"]
chrome-gtk4 = ["dep:gtk4"]
```

---

## Module Layout

```
light-speed-desktop/src/
├── main.rs              # Entry: backend detection, calloop setup, run loop
├── config.rs            # User config (TOML), breakpoint definitions
├── error.rs             # LsdError type hierarchy
├── state.rs             # GlobalState: owns all subsystem state
├── compositor/
│   ├── mod.rs           # Smithay CompositorHandler impl
│   ├── surface.rs       # Surface role dispatch, damage tracking
│   └── output.rs        # Output management, DPI/scale
├── wm/
│   ├── mod.rs           # WindowManager: layout dispatch
│   ├── layout.rs        # LayoutEngine trait
│   ├── stacking.rs      # StackingLayout: floating windows, z-order
│   ├── tiling.rs        # TilingLayout: binary BSP or grid
│   ├── responsive.rs    # BreakpointSet: selects layout per output geometry
│   └── workspace.rs     # Workspace: collection of windows per output
├── input/
│   ├── mod.rs           # InputHandler: seat management
│   ├── pointer.rs       # Pointer events, cursor theme
│   ├── touch.rs         # Touch events → Wayland touch protocol
│   ├── keyboard.rs      # Keyboard: xkbcommon, key bindings
│   └── gesture.rs       # Gesture recognizer (swipe workspaces, pinch)
├── chrome/
│   ├── mod.rs           # ChromeManager trait; feature-flag dispatch
│   ├── iced/
│   │   ├── mod.rs       # Iced application shell
│   │   ├── panel.rs     # Taskbar / top panel
│   │   └── launcher.rs  # App launcher overlay
│   └── gtk4/
│       ├── mod.rs       # GTK4 LayerShell chrome
│       ├── panel.rs
│       └── launcher.rs
├── backend/
│   ├── mod.rs           # BackendHandle trait
│   ├── drm.rs           # DRM/KMS backend (bare metal)
│   ├── winit.rs         # Winit backend (nested / development)
│   └── waymux.rs        # Waymux virtual output backend (for Bridge capture)
├── screencopy/
│   ├── mod.rs           # wlr-screencopy-unstable-v1 global handler
│   └── capture.rs       # Frame capture state machine
└── xwayland/
    ├── mod.rs           # XWayland integration (Smithay xwayland module)
    └── surface_map.rs   # X11 window ↔ Wayland surface mapping
```

---

## Responsive Layout System

LSD defines layout breakpoints based on the **logical resolution** of each output:

| Breakpoint | Condition | Layout Mode | Chrome |
|---|---|---|---|
| `Phone` | width < 600 dp | Fullscreen stacking | Minimal bottom bar |
| `Tablet` | 600 ≤ width < 1200 dp | Tiling (2 columns) | Side panel optional |
| `Desktop` | width ≥ 1200 dp | Hybrid: tiling primary, floating secondary | Full panel + launcher |

Breakpoints are re-evaluated on:
- Output creation or reconfiguration.
- Output rotation (width/height swap).
- User-initiated DPI override in config.

The `BreakpointSet` type maps an `Output` to a `LayoutMode` and a `ChromeMode`. The `WindowManager` holds one `LayoutEngine` instance per output, swapping it when the breakpoint changes.

---

## Window Manager

### StackingLayout

- Windows have explicit z-order.
- Move and resize via pointer drag on title bar (LSD draws a minimal server-side decoration via `xdg-decoration`).
- New windows appear centered, or at the last cursor position on large displays.
- Minimized windows are tracked in a hidden stack; shown in the taskbar.

### TilingLayout

- Binary space partitioning (BSP): each split is either horizontal or vertical.
- New windows split the focused tile.
- No floating windows in pure tiling mode; modal dialogs override to floating.
- Keyboard shortcuts: `Super+H/V` to split, `Super+Arrows` to navigate tiles, `Super+W` to close.

### Hybrid Mode (Desktop breakpoint)

- Primary tiling zone occupies the majority of the screen.
- A floating layer sits above the tiling zone.
- Windows can be toggled between tiling and floating with `Super+T`.
- Dialog and popup surfaces always open as floating.

---

## Input Handling

- Seat management via Smithay's `SeatHandler`.
- Keyboard: `xkbcommon` for keymap, key repeat with configurable rate/delay.
- Pointer: cursor themes loaded from `XCURSOR_THEME` / `XCURSOR_SIZE` env vars.
- Touch: forwarded via `wl_touch` to focused surface; gesture recognizer runs in parallel:
  - Three-finger swipe left/right: switch workspace.
  - Pinch on background: open launcher.
- Hardware keyboard shortcuts are defined in `config.rs` as a `KeyBindingMap`.

---

## Screencopy Extension

LSD implements `zwlr_screencopy_manager_v1` as a Wayland global. This allows Waymux Bridge to capture frames.

Frame capture flow (compositor side):
1. Bridge sends `zwlr_screencopy_manager_v1::capture_output` request.
2. LSD creates a `ScreencopyFrame` object.
3. On the next render cycle after a `copy` request, LSD blits the output framebuffer into the shared memory buffer provided by the Bridge.
4. LSD sends `zwlr_screencopy_frame_v1::ready` with a timestamp.
5. LSD sends `damage` events for regions that changed since the last frame (requires tracking per-client damage state).

---

## Backend System

### DRM/KMS Backend

- Uses Smithay's `DrmBackend` and `UdevBackend`.
- Enumerates GPUs via udev, opens DRM devices, creates CRTC/connector/plane assignments.
- Supports hardware plane overlay for cursor (avoids full composite for cursor movement).
- Requires running as root or with `CAP_SYS_ADMIN`, or under a Wayland session manager with a logind session.

### Winit Backend

- Used for development and testing on a host desktop.
- Single virtual output matching the Winit window size.
- Keyboard/pointer events from the host desktop are injected into the LSD seat.

### Waymux Virtual Output Backend

- A headless output with configurable resolution and refresh rate.
- Framebuffer is allocated in shared memory.
- On frame commit, signals the screencopy subsystem to copy the framebuffer to any waiting Bridge clients.
- Allows LSD to run without a physical display — pure "server mode" for Termux.

---

## Configuration File

Location: `$XDG_CONFIG_HOME/light-speed-desktop/config.toml` (default: `~/.config/light-speed-desktop/config.toml`)

```toml
[general]
chrome = "iced"          # "iced" | "gtk4"
default_layout = "auto"  # "auto" | "stacking" | "tiling"

[breakpoints]
phone_max_width   = 600
tablet_max_width  = 1200

[input]
keyboard_repeat_rate  = 30    # repeats per second
keyboard_repeat_delay = 500   # ms before first repeat
xcursor_theme = "Adwaita"
xcursor_size  = 24

[xwayland]
enabled = true

[[keybinding]]
keys = ["Super", "Return"]
action = "spawn"
args  = ["foot"]

[[keybinding]]
keys = ["Super", "Q"]
action = "close_focused"

[[keybinding]]
keys = ["Super", "T"]
action = "toggle_floating"
```

---

## XWayland Integration

- Spawns the `Xwayland` binary (path from `XWAYLAND_PATH` env or `PATH` lookup).
- Smithay's `XWayland` helper manages the lifecycle and the `DISPLAY` env var.
- `surface_map.rs` maintains a bidirectional map between X11 window IDs and `WlSurface` objects.
- X11 windows are wrapped in synthetic `xdg_toplevel` roles for WM integration.
- `_NET_WM_STATE_FULLSCREEN` is honoured by switching to fullscreen stacking layout for the requesting window.

---

## Testing Requirements

- `tests/layout_stacking.rs`: place 3 windows, verify z-order, move one, verify new z-order.
- `tests/layout_tiling.rs`: open 4 windows, verify BSP split geometry, close one, verify reflow.
- `tests/responsive_breakpoints.rs`: simulate output resize events crossing breakpoints; verify layout mode transitions.
- `tests/config_parsing.rs`: round-trip config TOML serialize/deserialize; verify keybinding parsing.
- `tests/screencopy_protocol.rs`: use Smithay test harness to bind the screencopy global, request a frame, verify `ready` event fires.
- Use `cargo test --features chrome-iced` and `cargo test --features chrome-gtk4` separately.
- Use Smithay's `wayland-server` in-process test infrastructure for all protocol tests.
