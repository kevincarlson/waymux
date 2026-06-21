# Waymux Client (Android)

The Android app that displays the Waymux Bridge's frame stream and forwards
input. It is a thin Kotlin shell over the Rust library `waymux-client`
(`libwaymux_client.so`), which owns the socket connection, frame decoding, wgpu
rendering, and input serialization.

> Status: builds an installable APK that renders the bridge's frame stream
> (including the built-in **test-pattern** source) and forwards touch, mouse,
> scroll, and key input. See `../waymux-client/docs/spec.md` for the full design.

---

## Prerequisites

Install on your build machine (Windows, macOS, or Linux):

1. **Android Studio** (Koala / 2024.1+), with from the SDK Manager:
   - **Android SDK Platform 35**
   - **Android SDK Build-Tools 35**
   - **NDK (Side by side)** — e.g. `27.0.12077973`
   - **CMake** (optional but recommended)
2. **Rust** via [rustup](https://rustup.rs/), then the Android targets:
   ```
   rustup target add aarch64-linux-android x86_64-linux-android
   ```
3. **cargo-ndk** (drives the cross-compile and NDK linker setup):
   ```
   cargo install cargo-ndk
   ```
4. Set **`ANDROID_NDK_HOME`** to your installed NDK, e.g. on Windows
   (PowerShell, adjust the version):
   ```powershell
   setx ANDROID_NDK_HOME "$env:LOCALAPPDATA\Android\Sdk\ndk\27.0.12077973"
   ```
   Restart the terminal / Android Studio afterwards so the variable is picked up.

`local.properties` (SDK location) is normally created by Android Studio on first
open; see `local.properties.example` if you build from the command line.

---

## How the native library is built

`app/build.gradle.kts` registers a `cargoNdkBuild` Gradle task that runs:

```
cargo ndk -t arm64-v8a -t x86_64 \
    -o app/src/main/jniLibs build --release -p waymux-client
```

from the workspace root (the parent of this directory). It compiles the Rust
crate for both ABIs and drops the `.so` files into `app/src/main/jniLibs/<abi>/`,
which Gradle then packages into the APK. The task is wired to run before
`preBuild`, so a normal Gradle build compiles the Rust automatically.

---

## Build

### Android Studio (recommended)
1. `git clone` the repository and open the **`waymux-client-android`** folder in
   Android Studio (open this subfolder, not the repo root).
2. Let Gradle sync. If prompted about the Gradle wrapper, accept it (Studio
   provisions Gradle 8.9 from `gradle/wrapper/gradle-wrapper.properties`).
3. Run the **app** configuration onto a device/emulator, or
   **Build > Build APK(s)**.

### Command line
The repo does not ship the Gradle wrapper JAR. Either open once in Android
Studio (which creates it), or, with a system Gradle 8.9+ installed:
```
cd waymux-client-android
gradle wrapper          # one-time, creates ./gradlew(.bat)
./gradlew assembleDebug  # or gradlew.bat on Windows
```
The APK lands in `app/build/outputs/apk/debug/app-debug.apk`.

---

## Run

1. On the Android device, install and start **Termux**, and run the
   **`waymux-bridge`** there (it currently streams an animated test pattern):
   ```
   waymux-bridge --encoding zstd
   ```
   It listens on `$TMPDIR/waymux.sock`
   (`/data/data/com.termux/files/usr/tmp/waymux.sock`).
2. Install and launch this app. By default it connects to that path. To point at
   a different socket:
   ```
   adb shell am start -n app.appthere.waymux/.MainActivity \
       --es waymux_socket /data/data/com.termux/files/usr/tmp/waymux.sock
   ```
3. You should see the streamed frames. Touch, mouse, scroll, and hardware-key
   input are forwarded to the bridge.

> **Socket access:** reaching Termux's socket from a separate app requires the
> app and Termux to share a `sharedUserId` (same signing key) or an equivalent
> Termux access grant. This is an integration/runtime concern, not a build one;
> see ADR-001 and the client spec.

---

## Customising versions

- **NDK version:** set via `ANDROID_NDK_HOME`; no Gradle change needed.
- **ABIs:** edit `rustAbis` in `app/build.gradle.kts` (and `ndk.abiFilters`).
- **SDK levels:** `compileSdk`/`minSdk`/`targetSdk` in `app/build.gradle.kts`.

## Troubleshooting

- *`cargo: command not found` during Gradle build* — ensure Rust/`cargo` and
  `cargo-ndk` are on the `PATH` of the shell that launched Android Studio.
- *Linker / NDK errors from cargo-ndk* — confirm `ANDROID_NDK_HOME` points at a
  real NDK and the `*-linux-android` rust targets are installed.
- *`UnsatisfiedLinkError: libwaymux_client.so`* — the `cargoNdkBuild` task did
  not produce a `.so` for the device ABI; run `./gradlew cargoNdkBuild` and check
  its output, and verify `app/src/main/jniLibs/<abi>/libwaymux_client.so` exists.
