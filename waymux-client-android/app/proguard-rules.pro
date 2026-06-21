# Keep the JNI bridge: its method names must match the Rust `#[no_mangle]`
# exports in `libwaymux_client.so`.
-keepclasseswithmembernames class app.appthere.waymux.RustBridge {
    native <methods>;
}
-keep class app.appthere.waymux.RustBridge { *; }
