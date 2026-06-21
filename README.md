# Waymux

**A Wayland desktop on your Android device, served from Termux.**

Waymux routes graphical output from a Wayland compositor running in
[Termux](https://termux.dev/) to a native Android app, and forwards touch,
mouse, and keyboard input back. It lets you run a real Linux desktop on a phone
or tablet and interact with it on the device's screen.

The repository also intends to ship **Light Speed Desktop**, a lightweight
Wayland compositor and desktop environment as the first-class capture target
(not yet started).

> **Status:** the M1–M4 software stack is implemented. The protocol, the bridge
> (capture + encode + serve + pointer injection), and the Android client
> (decode + render + input) all build and pass CI. Runtime validation on a real
> device / compositor is in progress — see [Implementation status](#implementation-status).

---

## How it works

```
┌──────────────────────── Android device ────────────────────────┐
│                                                                 │
│   ┌─────────────── Android app (waymux-client-android) ──────┐  │
│   │  MainActivity + SurfaceView (Kotlin)                     │  │
│   │      │  JNI                                              │  │
│   │  libwaymux_client.so (Rust): decode · wgpu render ·      │  │
│   │                              input serialize             │  │
│   └──────────────┬───────────────────────▲──────────────────┘  │
│         frames   │   Unix domain socket   │  input              │
│   ─ ─ ─ ─ ─ ─ ─ ─│─ ─ ($TMPDIR/waymux.sock)│─ ─ ─ ─ ─ ─ ─ ─ ─    │
│                  ▼                        │                      │
│   ┌──────────────────── Termux (Linux) ──┴──────────────────┐   │
│   │  waymux-bridge (Rust daemon)                            │   │
│   │   • capture: wlr-screencopy  • encode: raw / zstd       │   │
│   │   • serve frames over the socket                        │   │
│   │   • inject input: wlr-virtual-pointer                   │   │
│   └──────────────┬──────────────────────────────────────────┘  │
│                  │  $WAYLAND_DISPLAY                             │
│                  ▼                                               │
│   ┌─────────────────────────────────────────────────────────┐  │
│   │  Any wlroots compositor (sway, labwc, …) — or the        │  │
│   │  built-in test pattern when no compositor is available   │  │
│   └─────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

Frames flow Bridge → Client over the **Waymux Frame Protocol (WFP)**; input
flows Client → Bridge over the **Waymux Input Protocol (WIP)**. Both are defined
in `waymux-proto` (length-prefixed binary framing). See
[`docs/spec.md`](docs/spec.md) for the full design.

---

## Components

| Directory | Crate / project | Role |
|---|---|---|
| [`waymux-proto/`](waymux-proto) | Rust lib | WFP + WIP message types and codec (the wire contract) |
| [`waymux-bridge/`](waymux-bridge) | Rust bin | Termux daemon: capture, encode, serve frames, inject input |
| [`waymux-client/`](waymux-client) | Rust lib + cdylib | Client core (decode/input/transport/state) + Android JNI & wgpu renderer |
| [`waymux-client-android/`](waymux-client-android) | Android (Kotlin) | The Android app shell ([build instructions](waymux-client-android/README.md)) |
| `light-speed-desktop/` | Rust bin | Wayland compositor + DE (specced, not yet implemented) |

Per-package specs live in each directory's `docs/spec.md`; architecture
decisions are in [`docs/adr/`](docs/adr).

---

## Implementation status

Milestones (from [`docs/spec.md`](docs/spec.md) §10):

| Milestone | Scope | State |
|---|---|---|
| M1 | `waymux-proto` codec + tests | ✅ Done, fully tested |
| M2 | Bridge captures a compositor, streams frames | ✅ Code complete; ⏳ on-device capture not yet validated |
| M3 | Client renders frames on Android | ✅ Code complete; ⏳ on-device render not yet validated |
| M4 | Bidirectional input | ◑ Pointer/touch/scroll injection done; keyboard/stylus pending |
| M5 | zstd + damage regions | ◑ zstd done; damage regions pending |
| M6–M10 | Light Speed Desktop | ◻ Not started |

What this means concretely:

- **Verified on the host (CI):** the protocol, the bridge's encode/serve
  pipeline with the **test-pattern source**, and the client's
  decode/input/transport/state core — including a live bridge↔client loop over a
  real socket.
- **Compiles, needs a device/compositor to validate:** the bridge's
  `wlr-screencopy` capture and `wlr-virtual-pointer` injection (need a wlroots
  compositor in Termux), and the Android JNI + wgpu renderer + Kotlin app (need
  the Android NDK/SDK and a device).
- **Not yet implemented:** keyboard/stylus injection (needs an xkb keymap +
  Android→evdev mapping), damage-region capture, and Light Speed Desktop.

---

## Build (Rust workspace)

Requires a recent stable Rust (edition 2024). On a normal Linux host:

```sh
cargo build --workspace
cargo test  --workspace
```

The Android library and the bridge's Wayland backend compile on the host too;
the Android app is built separately (below).

### Bridge configuration

`waymux-bridge` is configured by flags or environment variables:

| Flag | Env | Default | Description |
|---|---|---|---|
| `--source` | `WAYMUX_SOURCE` | `wayland` | `wayland` (capture a compositor) or `test-pattern` |
| `--socket` | `WAYMUX_SOCKET` | `$TMPDIR/waymux.sock` | Unix socket to listen on |
| `--encoding` | `WAYMUX_ENCODING` | `zstd` | `raw` or `zstd` |
| `--zstd-level` | `WAYMUX_ZSTD_LEVEL` | `3` | zstd level (1–22) |
| `--max-fps` | `WAYMUX_MAX_FPS` | `60` | Frame-rate cap |
| `--overlay-cursor` | `WAYMUX_OVERLAY_CURSOR` | `true` | Draw the cursor into frames (Wayland source) |
| `--width` / `--height` | `WAYMUX_WIDTH` / `WAYMUX_HEIGHT` | `1280` / `720` | Test-pattern geometry |
| — | `WAYMUX_LOG` | `info` | `tracing` log filter |

---

## Run it end to end

### Option A — no compositor needed (quickest demo)

Stream the built-in animated test pattern and decode it with the bundled example
client. Great for trying the pipeline or developing the Android app.

```sh
# Terminal 1: serve the test pattern (raw, small, so it's easy to inspect)
cargo run -p waymux-bridge -- --source test-pattern \
    --socket /tmp/waymux.sock --encoding raw --width 320 --height 240

# Terminal 2: connect a client and print the first decoded frame
cargo run -p waymux-client --example dump_frame -- /tmp/waymux.sock
```

You can also point the **Android app** at this socket to see the test pattern
render on a device.

### Option B — capture a real compositor (the real thing, in Termux)

On the Android device, inside Termux:

```sh
# 1. Start a wlroots compositor (example: sway, headless or with an output).
#    Any compositor advertising wlr-screencopy + wlr-virtual-pointer works.
export WAYLAND_DISPLAY=wayland-1
sway &                       # or labwc, etc.

# 2. Run the bridge (defaults to --source wayland).
waymux-bridge --encoding zstd
#    It listens on $TMPDIR/waymux.sock and exits with code 3 + a clear message
#    if no compositor is found.
```

Then launch the **Waymux** Android app, which connects to
`/data/data/com.termux/files/usr/tmp/waymux.sock` by default and renders the
compositor's output. Touch, mouse, and scroll are injected back into the
compositor.

> **Socket access:** reaching Termux's socket from a separate app requires the
> app and Termux to share a `sharedUserId` (same signing key) or an equivalent
> Termux access grant — see [ADR-001](docs/adr/adr-00001.md) and the client
> spec. This is a packaging/runtime concern, not a build one.

### Building the Android app

See **[`waymux-client-android/README.md`](waymux-client-android/README.md)** for
the full workflow (Android Studio, Rust Android targets, `cargo-ndk`, deploying
to a device). In short: install the Android SDK + NDK and `cargo-ndk`, open
`waymux-client-android/` in Android Studio, and Run.

---

## Repository layout

```
waymux/
├── Cargo.toml                  workspace root
├── README.md                   this file
├── AGENTS.md                   contributor / AI-assistant guide + code rules
├── docs/
│   ├── spec.md                 system specification
│   └── adr/                    architecture decision records
├── waymux-proto/               WFP/WIP protocol crate
├── waymux-bridge/              Termux daemon
├── waymux-client/              client core + Android JNI/renderer (lib + cdylib)
├── waymux-client-android/      Android app (Kotlin + Gradle)
└── light-speed-desktop/        compositor (specced, not yet implemented)
```

---

## Development

CI (`.github/workflows/ci.yml`) runs the same gates every change must pass:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```

The project's non-negotiable code rules (no `unwrap`/`expect` or `unsafe` in
library code outside FFI, 300-line file limit, TDD, full rustdoc, Rust 2024) are
described in [`AGENTS.md`](AGENTS.md). New dependencies require an ADR in
[`docs/adr/`](docs/adr).

---

## License

Apache-2.0. See [`LICENSE`](LICENSE).
