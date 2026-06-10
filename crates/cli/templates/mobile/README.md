# Glory mobile targets (Android / iOS)

Status: **toolchain orchestration + host templates shipped; cross-compile
verified** (`examples/mobile-counter` produces a real arm64 `.so` via
cargo-ndk on the reference machine). On-device run remains to be
exercised in CI.

Working pieces:
- `examples/mobile-counter` — a complete mobile payload crate: shared
  Counter widget + mini webview host loop + the tao/wry `android_binding!`
  JNI wiring and the iOS `start_app` export. Start by copying it.
- `android/` (this directory) — Gradle host project template. The
  `cargoNdk` Gradle task builds the Rust cdylib AND lets wry's build
  script generate the Kotlin glue (WryActivity etc.) with your package
  name substituted; `MainActivity` just subclasses it.
- `ios/` — XcodeGen spec + Swift entry calling the Rust `start_app`.

The runtime story is the same as desktop: the widget tree runs on the
command-stream backend (`glory-core` feature `backend-command`), a wry
webview applies `Command` batches via `glory_desktop::WRY_INTERPRETER_JS`,
and DOM events come back as serialized `EventData` (touch events map to
`PointerData`; multi-touch points ride in `extra.touches`).

## Android

Prerequisites:

```sh
cargo install cargo-ndk
rustup target add aarch64-linux-android
# set ANDROID_NDK_HOME to your NDK root
```

Build:

```sh
glory build --target android --release
```

This produces `target/aarch64-linux-android/release/lib<yourlib>.so` (also
copied under `<site-root>/android/jniLibs/arm64-v8a/` by cargo-ndk's `-o`).

Host-project wiring (Gradle):

1. Create a standard Android app project (empty Activity).
2. Drop the `jniLibs` directory into `app/src/main/`.
3. Your crate must expose the wry Android entry point — add to the lib:

   ```rust
   #[cfg(target_os = "android")]
   #[unsafe(no_mangle)]
   fn android_main(app: tao::platform::android::activity::AndroidApp) {
       wry::android_binding!(com_example, your_app, _start_app, wry);
       // then glory_desktop::launch(...) on the tao android event loop
   }
   ```

4. The Activity loads the library via `System.loadLibrary("yourlib")`.

Consult the wry repository's `examples/android` for the authoritative,
version-matched binding macro invocation — it changes between wry releases.

## iOS

macOS host with Xcode required:

```sh
rustup target add aarch64-apple-ios
glory build --target ios --release
```

This produces `target/mobile/aarch64-apple-ios/release/lib<yourlib>.a`.
Link the static library into an Xcode (or XcodeGen) project whose
`main` calls into your exported `start_app` symbol; wry creates a
`WKWebView` inside the key window.

## Cargo manifest requirements

The lib package needs the mobile crate types alongside the wasm one:

```toml
[lib]
crate-type = ["cdylib", "staticlib", "rlib"]

[features]
mobile = ["glory/backend-command", "dep:glory-desktop"]
```

`glory build --target android|ios` selects the `mobile` feature by default
(override with `--lib-features`).
