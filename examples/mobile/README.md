# Mobile counter

Android NativeActivity and iOS UIKit hosts mount the same `counter::Counter`
component used by Web and desktop. The application owns its entry point and
event loop; no process-global AndroidApp or second reactive runtime is installed.
The standalone workspace depends on the existing facade and counter library,
so the runtime workspace retains twelve packages.

Android uses the shell's CPU renderer and winit's `android-native-activity` glue.
`Shell::android(app)` requires the AndroidApp passed to this invocation's
`android_main`. `Shell::from_event_loop` also accepts an explicitly configured
event loop. iOS uses `Shell::new` on the application's UIKit main thread.
The runtime's optional desktop `assets` and `accesskit` integrations remain
desktop-only; enabling those flags does not install mobile implementations.

## Android

Install an Android SDK, NDK, JDK and the Rust target first. The build script reads
these existing tools; it does not install SDK packages or modify user tool homes.
All output, compiler caches, Python caches, Java homes, debug signing keys and
emulator writable images stay under `D:/BuildCache/glory-mobile`.

```powershell
./examples/mobile/build-android.ps1
# Optional arm64 package for a device; this is a build, not a device test.
./examples/mobile/build-android.ps1 -Target aarch64-linux-android
```

`-Sdk`, `-Ndk`, `-JavaHome` and `-BuildRoot` override tool locations and output.
The default SDK comes from `ANDROID_HOME`. If `ANDROID_NDK_HOME` is absent or
stale, the newest installed NDK in that SDK is selected. Cargo's actual target
directory is checked before compilation. The committed Cargo.lock pins the
application; ordinary builds use `--locked`.

The script creates a native library, packages AndroidManifest.xml with the
SDK's aapt, aligns the APK and signs it with a D:-local debug key. Output is
`<build-root>/package-<target>-<debug|release>/glory-counter.apk` plus `build.json`
with its ABI and SHA-256. Both profiles use debug signing; store distribution,
release credentials and production manifest policy are separate work.
The single `#[unsafe(no_mangle)]` on the application entry is the explicit
user-authorized exception recorded in [AGENTS.md](../../AGENTS.md); it exports
the Android ABI symbol and introduces no unsafe block or runtime exception.

For a disposable Android 16 x86_64 emulator with the existing Google Play image:

```powershell
./examples/mobile/start-emulator.ps1
$env:ANDROID_USER_HOME = 'D:/BuildCache/glory-mobile/android-home'
$env:TEMP = 'D:/BuildCache/glory-mobile/tmp'; $env:TMP = $env:TEMP
$env:PYTHONPYCACHEPREFIX = 'D:/BuildCache/glory-mobile/pycache'
& "$env:ANDROID_HOME/platform-tools/adb.exe" -s emulator-5580 install --no-incremental -r D:/BuildCache/glory-mobile/package-x86_64-linux-android-debug/glory-counter.apk
python examples/mobile/accept-android.py --serial emulator-5580 --out D:/BuildCache/glory-mobile/acceptance
```

Wait for that emulator to finish booting before installing. Its system image is
read from the existing SDK; AVD config, userdata, snapshots/cache/temp and logs
are isolated on D:. The script returns a launcher PID; stop only the owned
`emulator-5580` with `adb -s emulator-5580 emu kill` when finished.

The acceptance script requires Pillow and the specified 1080×2400, 420-DPI
fixture with Gboard's light number row. It locates the keyboard's actual top
instead of assuming the Android Studio profile's keyboard height, and temporarily
enables software input alongside a hardware keyboard. It verifies visible touch updates, injected editing,
system keyboard visibility and an actual software-key tap, same-process
background resume, rotation restoration, subsequent input, and Activity close
and recreation inside the same process. Rotation waits for a final frame;
Android's transition may briefly scale the old buffer. Full screenshots and
application logs are retained alongside the result. These tests do not establish
CJK composition, TalkBack, device permissions, network/asset transport or real
arm64-device support.

The Winit fork retires the Activity loop on Destroy and holds an exclusive
permit until that loop's fields drop. A new Activity can then create its own
loop; overlapping loops remain rejected. Versions and fork-owned tests are in
[engine-forks.json](../../docs/engine-forks.json).

## iOS

`src/ios.rs` and `ios/Info.plist` define the application entry and bundle.
On a Mac with Xcode and the simulator Rust target:

```sh
export GLORY_BUILD_ROOT=/Volumes/D/BuildCache/glory-mobile
sh examples/mobile/build-ios.sh
xcrun simctl install booted "$GLORY_BUILD_ROOT/ios-aarch64-apple-ios-sim/GloryCounter.app"
xcrun simctl launch booted rs.glory.counter
```

The build root must be inside BuildCache on a verified Windows administrative
`D$` SMB mount, following this repository's storage policy. The script rejects
other storage and non-macOS hosts, checks Cargo metadata, uses the simulator SDK
and creates an ad-hoc-signed simulator `.app`. Intel Macs can set
`GLORY_IOS_TARGET=x86_64-apple-ios`. The current Windows session has no Apple SDK;
the iOS path remains uncompiled and unaccepted. Device provisioning, signed IPA,
UIKit lifecycle/input/safe areas, VoiceOver and simulator/device runtime checks
remain required before claiming iOS support.

The Windows canonical gate accepts `-Android` to check the adapter and build both
APKs. Add `-AndroidSerial emulator-5582` for an explicitly selected, already
running owned emulator; APK builds alone are reported separately from runtime
acceptance. The native adapter check uses `--no-deps` because the current Android
Clippy diagnoses already-const thread locals in core.
