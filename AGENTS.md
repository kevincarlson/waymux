# AGENTS.md — AppThere Waymux

This file instructs AI coding assistants (Claude Code, Cursor, GitHub Copilot, etc.) on how to contribute effectively to this repository.

---

## Repository Overview

AppThere Waymux is a Rust-first monorepo implementing a Wayland compositor output forwarding system for Android/Termux. It contains four Rust crates and one Android Studio project:

| Directory | Type | Description |
|---|---|---|
| `waymux-proto/` | Rust lib | Shared protocol types (WFP + WIP) |
| `waymux-bridge/` | Rust bin | Termux daemon, Wayland client + IPC server |
| `waymux-client-rs/` | Rust cdylib | Android JNI library (rendering + input) |
| `waymux-client-android/` | Android/Kotlin | Activity shell + build integration |
| `light-speed-desktop/` | Rust bin | Wayland compositor + DE |

Read the relevant `SPEC.md` in each directory before making changes to that package. Read `SPEC.md` (root) for the full system design.

---

## Non-Negotiable Code Rules

These rules are **enforced by CI** and must never be violated:

1. **No `unwrap()` or `expect()`** in library code (`waymux-proto`, `waymux-bridge`, `waymux-client-rs`, `light-speed-desktop`). Use `?`, `match`, or typed `Result` returns. `main.rs` may use `color-eyre`'s `eyre!` macro.

2. **No `unsafe` blocks** unless at an FFI boundary (JNI in `waymux-client-rs`). Every `unsafe` block **must** have a `// SAFETY:` comment immediately above it explaining all invariants upheld. Do not add `unsafe` to work around borrow-checker issues — redesign instead.

3. **No excessive `.clone()`**. For large data (frame buffers, strings, `Vec<u8>` payloads), prefer `Arc<[u8]>`, slices, or owned-once patterns. If you must clone something large, add a `// PERF: clone justified because ...` comment.

4. **300-line file limit**. If a file is approaching 300 lines, split it. Group by single responsibility: one file per protocol message family, one file per compositor subsystem, etc.

5. **TDD**: Write the test first or alongside the implementation. Every public function needs at least one `#[test]`. Don't add stubs; don't leave `todo!()` in committed code.

6. **Documentation**: Every `pub` item needs a `///` rustdoc comment. Every module needs a `//!` header. Comments explain *why*, not *what*.

7. **Rust 2024 edition** — use modern idioms. No deprecated APIs.

---

## Architecture Constraints

- Do **not** reach for a new dependency without checking if an existing workspace dependency already covers the need. See `SPEC.md §8` for the approved dependency list.
- Adding a new dependency requires an ADR entry in `docs/adr/` — create one as part of the same PR.
- The `waymux-proto` crate must have **zero** dependencies on `smithay`, `wgpu`, or `tokio`. It may use `thiserror`, `serde` (behind a feature flag), and `bytes`.
- The `waymux-bridge` crate must be cross-compilable to `aarch64-linux-android` (Termux). Do not use crates that require `std::net::TcpStream` on Android without `cfg` guards; use `tokio::net::UnixStream`.
- The `waymux-client-rs` crate compiles as a `cdylib`. Its public API surface is only JNI-exported functions (`#[no_mangle] pub extern "system" fn Java_...`). Do not expose other pub items except via Rust-internal modules.

---

## Module Conventions

### Protocol (waymux-proto)

- Message types live in `src/wfp.rs` (Bridge→Client) and `src/wip.rs` (Client→Bridge).
- Codec logic (encode/decode) lives in `src/codec.rs`.
- All message enums use `#[repr(u8)]` discriminants matching the protocol spec in `SPEC.md §4`.
- Round-trip tests: every message type must have an encode→decode test in `tests/round_trip.rs`.

### Bridge (waymux-bridge)

- `src/main.rs`: entry point only (arg parsing, runtime setup, top-level error reporting). Max 80 lines.
- `src/compositor/`: Wayland client interaction (screencopy, virtual input injection).
- `src/server/`: Unix socket server, client session management.
- `src/encoder/`: Frame encoding pipeline.
- `src/config.rs`: Config struct (parsed from env + CLI).

### Client-RS (waymux-client-rs)

- `src/lib.rs`: JNI exports only. Max 100 lines.
- `src/renderer/`: wgpu frame rendering pipeline.
- `src/decoder/`: WFP frame decoding.
- `src/input/`: Input event serialization to WIP.
- `src/connection/`: Unix socket client (tokio).

### Light Speed Desktop

- `src/main.rs`: entry point. Max 80 lines.
- `src/compositor/`: Smithay state, surface management, output management.
- `src/wm/`: Window manager logic (stacking, tiling, layout algorithm).
- `src/input/`: Input device handling, gesture recognition.
- `src/chrome/`: DE UI (Iced or GTK4 panels, launchers).
- `src/backend/`: Output backends (DRM/KMS, Winit, Waymux virtual output).
- `src/xwayland/`: XWayland integration.
- `src/config.rs`: User configuration.

---

## Testing Strategy

- Unit tests: `#[cfg(test)]` modules in the same file as the code under test.
- Integration tests: `tests/` directory at each crate root.
- Use `tokio::test` for async tests.
- For protocol tests, use property-based testing with `proptest` for message encode/decode round-trips.
- For compositor tests, use Smithay's test helpers and a headless backend.
- Do not use `sleep` in tests. Use `tokio::sync::barrier` or channel synchronization.

---

## Commit and PR Guidance

- One logical change per commit. Do not mix protocol changes with compositor changes.
- Commit messages: `<crate>: <imperative verb> <what and why>`. Example: `waymux-proto: add StylusMotion message for pen pressure forwarding`.
- Every PR must pass: `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, `cargo doc --no-deps --all-features`.
- If you add a dependency, update `Cargo.toml` workspace `[dependencies]` and create an ADR.

---

## Common Pitfalls to Avoid

- **Do not** use `tokio::spawn` inside Smithay's calloop callbacks without using a channel to bridge. Use `calloop::channel::Channel` or `calloop`'s Tokio compatibility layer.
- **Do not** access Android shared filesystem paths with hardcoded strings. Use the `WAYMUX_SOCKET` env var or derive from `TMPDIR`.
- **Do not** assume the compositor supports `wlr-screencopy`; check global advertisement at startup and return a descriptive error if absent.
- **Do not** hold frame buffer locks across `await` points.
- **Do not** use `std::sync::Mutex` in async contexts; use `tokio::sync::Mutex`.
- **Do not** open the Wayland socket from a tokio thread; use `calloop` or a dedicated blocking thread with a channel.

---

## Where to Ask for Guidance

If you are uncertain about:
- Protocol design → see `waymux-proto/SPEC.md` and `docs/adr/ADR-001-transport.md`, `ADR-002-frame-encoding.md`
- Compositor architecture → see `light-speed-desktop/SPEC.md` and `docs/adr/ADR-003-compositor-library.md`
- Android rendering → see `waymux-client-android/SPEC.md` and `docs/adr/ADR-004-android-rendering.md`
- Widget toolkit choice → see `docs/adr/ADR-005-widget-toolkit.md`
- Anything else → read `SPEC.md` at the root first, then the relevant sub-package `SPEC.md`
