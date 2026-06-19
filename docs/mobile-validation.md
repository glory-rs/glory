# Mobile Device Validation

This page records the runtime smoke path for Glory mobile artifacts. Compile
smoke runs in CI; real device validation still needs a local Android device,
Android emulator, iOS simulator, or iOS device.

Android emulator smoke is also wired as a manual/nightly GitHub Actions workflow
at `.github/workflows/mobile-device-smoke.yml`. It uses an API 34 x86_64
emulator and `GLORY_ANDROID_ABI=x86_64`.

## Android

Prerequisites:

- Android SDK platform tools on `PATH` (`adb`) or under
  `ANDROID_HOME` / `ANDROID_SDK_ROOT`.
- A booted emulator or attached device visible in `adb devices`.
- `cargo-ndk`, Android NDK, and the `aarch64-linux-android` Rust target.
  Set `GLORY_ANDROID_ABI=x86_64` and install `x86_64-linux-android` when
  validating on an x86_64 emulator.

Build and bundle a generated mobile app:

```powershell
cargo run -p glory-cli -- new --template mobile --name mobile-smoke
Set-Location mobile-smoke
cargo run --manifest-path ..\Cargo.toml -p glory-cli -- build --target android --release
cargo run --manifest-path ..\Cargo.toml -p glory-cli -- bundle --target android --release
```

Then install and launch the APK:

```powershell
..\scripts\mobile-device-smoke.ps1 `
  -Target android `
  -AndroidApk dist\mobile-smoke\android\apk\app-release.apk `
  -AndroidPackage com.example.mobile_smoke `
  -AndroidActivity .MainActivity
```

Set `GLORY_ANDROID_DEVICE` or pass `-AndroidSerial` when more than one device is
connected.

To verify the on-device reload websocket path, run the CLI in watch mode with a
known reload port, then let the smoke script configure adb reverse before it
launches the app:

```powershell
$env:GLORY_WATCH = "ON"
$env:GLORY_RELOAD_PORT = "3001"
$env:GLORY_ANDROID_REVERSE_RELOAD = "1"
..\scripts\mobile-device-smoke.ps1 `
  -Target android `
  -AndroidReverseReload `
  -ReloadPort 3001 `
  -AndroidApk dist\mobile-smoke\android\apk\app-debug.apk `
  -AndroidPackage com.example.mobile_smoke `
  -AndroidActivity .MainActivity
```

The generated Android bundle `run.ps1` / `run.sh` scripts also honor
`GLORY_ANDROID_REVERSE_RELOAD=1`. The mobile webview connects to
`ws://127.0.0.1:$GLORY_RELOAD_PORT/live_reload` by default; set
`GLORY_MOBILE_RELOAD_HOST` or `GLORY_MOBILE_RELOAD_URL` when the device must
reach the host over LAN instead. Rust code changes rebuild the mobile library
in watch mode, but still require reinstall/relaunch to load the new native
library in the running app.

## iOS

Prerequisites:

- macOS with Xcode command line tools.
- XcodeGen when using the generated `ios/project.yml`.
- A booted simulator or connected device.

Build and bundle:

```sh
cargo run -p glory-cli -- new --template mobile --name mobile-smoke
cd mobile-smoke
cargo run --manifest-path ../Cargo.toml -p glory-cli -- build --target ios --release
GLORY_IOS_SDK=iphonesimulator cargo run --manifest-path ../Cargo.toml -p glory-cli -- bundle --target ios --release
```

Then install and launch:

```sh
../scripts/mobile-device-smoke.ps1 \
  -Target ios \
  -IosApp dist/mobile-smoke/ios/app/MobileSmoke.app \
  -IosBundleId com.example.MobileSmoke
```

The script writes `target/mobile-device-smoke/mobile-device-smoke.json` with
`completed`, `blocked`, or `failed` status and per-step logs.

iOS simulator reload can usually use
`GLORY_MOBILE_RELOAD_URL=ws://127.0.0.1:3001/live_reload`. For physical iOS
devices, use a LAN-reachable host address such as
`ws://192.168.1.10:3001/live_reload`.

Current Windows host result, 2026-06-11: `examples/mobile-counter` host check
passes and `adb` is discovered at
`C:\Users\chris\AppData\Local\Android\Sdk\platform-tools\adb.exe`, but no
Android device/emulator is online. iOS validation is blocked on this host
because the iOS simulator requires macOS.
