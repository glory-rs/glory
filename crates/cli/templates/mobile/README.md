# Glory mobile targets (Android / iOS)

Status: **toolchain orchestration + host templates shipped; cross-compile
verified** (`examples/mobile-counter` produces a real arm64 `.so` via
cargo-ndk on the reference machine). On-device run remains to be
exercised in CI.

Working pieces:
- `examples/mobile-counter` — a complete mobile payload crate: shared
  Counter widget + mini webview host loop + the tao/wry `android_binding!`
  JNI wiring and the iOS `start_app` export. Start by copying it.
- `android/` - Gradle host project template. The
  `cargoNdk` Gradle task builds the Rust cdylib AND lets wry's build
  script generate the Kotlin glue (WryActivity etc.) with your package
  name substituted; `MainActivity` just subclasses it.
- `ios/` - XcodeGen spec + Swift entry calling the Rust `start_app`.

The runtime story is the same as desktop: the widget tree runs on the
command-stream backend (`glory-core` feature `backend-command`), a wry
webview applies `Command` batches via `glory_desktop::WRY_INTERPRETER_JS`,
and DOM events come back as serialized `EventData` (touch events map to
`PointerData`; multi-touch points ride in `extra.touches`).

## Mobile viewport and lifecycle defaults

Generated mobile crates use a bootstrap document with:

- `viewport-fit=cover`
- safe-area CSS variables:
  `--glory-safe-top`, `--glory-safe-right`, `--glory-safe-bottom`,
  `--glory-safe-left`
- keyboard/visual viewport variables:
  `--glory-viewport-height`, `--glory-keyboard-inset-bottom`
- browser custom events:
  `glory:viewport`, `glory:foreground`, `glory:background`

Android sets `windowSoftInputMode="adjustResize"` in the generated manifest so
the visual viewport can shrink when the soft keyboard opens. The bootstrap
script mirrors `visualViewport` resize/scroll into CSS variables; app CSS can
pad fixed footers with `var(--glory-keyboard-inset-bottom)`.

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
glory bundle --target android --release
```

This produces `target/aarch64-linux-android/release/lib<yourlib>.so` (also
copied under `<site-root>/android/jniLibs/arm64-v8a/` by cargo-ndk's `-o`).
`glory bundle` then runs `android/gradlew assembleRelease` (or `assembleDebug`
without `--release`) and copies APKs to:

```text
dist/<project>/android/apk/
```

The bundle also writes:

- `dist/<project>/android/install.ps1`
- `dist/<project>/android/run.ps1`
- `dist/<project>/android/install.sh`
- `dist/<project>/android/run.sh`

Useful knobs:

- `GLORY_ANDROID_GRADLE_TASK=assembleRelease` overrides the Gradle task list
  (comma or whitespace separated).
- `GLORY_ANDROID_INSTALL=1` adds `installRelease` / `installDebug`.
- `GLORY_ANDROID_RUN=1` launches the detected main activity through `adb`.
- `GLORY_ANDROID_DEVICE=<serial>` sets `ANDROID_SERIAL` for Gradle and is also
  used by the generated install/run scripts.

## iOS

macOS host with Xcode required:

```sh
rustup target add aarch64-apple-ios
glory build --target ios --release
glory bundle --target ios --release
```

This produces `target/mobile/aarch64-apple-ios/release/lib<yourlib>.a`.
`glory bundle` then runs `xcodegen generate` from `ios/`, runs `xcodebuild`,
and copies app bundles to:

```text
dist/<project>/ios/app/
```

If `GLORY_IOS_ARCHIVE=1` is set, the generated `.xcarchive` is copied to
`dist/<project>/ios/archive/`.

Useful knobs:

- `GLORY_IOS_SCHEME=<scheme>` overrides the scheme detected from
  `ios/project.yml`.
- `GLORY_IOS_SDK=iphonesimulator|iphoneos` selects simulator or device SDK.
- `GLORY_IOS_DESTINATION='<xcode destination>'` forwards an explicit
  xcodebuild destination.
- `GLORY_IOS_ARCHIVE=1` runs an archive step after the app build.

## Cargo manifest requirements

The lib package needs the mobile crate types alongside the wasm one:

```toml
[lib]
crate-type = ["cdylib", "staticlib", "rlib"]

[features]
mobile = []
```

`glory build --target android|ios` selects the `mobile` feature by default
(override with `--lib-features`).
