# Mobile Device Validation

This page records the runtime smoke path for Glory mobile artifacts. Compile
smoke runs in CI; real device validation still needs a local Android device,
Android emulator, iOS simulator, or iOS device.

## Android

Prerequisites:

- Android SDK platform tools on `PATH` (`adb`) or under
  `ANDROID_HOME` / `ANDROID_SDK_ROOT`.
- A booted emulator or attached device visible in `adb devices`.
- `cargo-ndk`, Android NDK, and the `aarch64-linux-android` Rust target.

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

Current Windows host result, 2026-06-11: `examples/mobile-counter` host check
passes and `adb` is discovered at
`C:\Users\chris\AppData\Local\Android\Sdk\platform-tools\adb.exe`, but no
Android device/emulator is online. iOS validation is blocked on this host
because the iOS simulator requires macOS.
