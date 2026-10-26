# Mobile counter

Android NativeActivity/GameActivity and iOS UIKit hosts mount the same `counter::Counter`
component used by Web and desktop. The application owns its entry point and
event loop; no process-global AndroidApp or second reactive runtime is installed.
The standalone workspace depends on the existing facade and counter library,
so the runtime workspace retains twelve packages.

Android uses the shell's CPU renderer with explicitly selected Winit Activity glue.
NativeActivity remains the default; GameActivity adds real GameTextInput composition.
`Shell::android(app)` requires the AndroidApp passed to this invocation's
`android_main`. `Shell::from_event_loop` also accepts an explicitly configured
event loop. iOS uses `Shell::new` on the application's UIKit main thread.
The application selects `blitz-package` and embeds `assets/theme.css` and
`assets/badge.png` as immutable `asset://app/` resources. Each document gets its
own bounded Loader, including a recreated Android Activity. Its imported CSS
positions and sizes the PNG badge; neither is supplied by a synchronous decode
shortcut. Queue/deadline/encoded-byte budgets and engine decode admission apply.
Package-only builds reject HTTP origins and file mounts and exclude desktop
HTTP/TLS/filesystem dependencies. Optional Android AssetManager support is described
below; networking, permissions and mobile accessibility remain separate gaps.
iOS has Apple-host build and simulator evidence; see the iOS boundaries below.

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
and recreation inside the same process. It checks the packaged PNG's actual
color and CSS-sized position before and after lifecycle changes, and the retired
document's Loader close log. Rotation waits for a final frame;
Android's transition may briefly scale the old buffer. Full screenshots and
application logs are retained alongside the result. These tests do not establish
CJK composition, TalkBack, device permissions, AssetManager/network transport or real
arm64-device support.

The Winit fork retires the Activity loop on Destroy and holds an exclusive
permit until that loop's fields drop. A new Activity can then create its own
loop; overlapping loops remain rejected. Versions and fork-owned tests are in
[engine-forks.json](../../docs/engine-forks.json).

## Android GameActivity on macOS

With the installed NDK r27d, SDK 36, JDK and Gradle 9.1.0:

```sh
GLORY_ANDROID_FEATURES=ime-probe sh examples/mobile/build-android-game.sh
# Ordinary shared Counter: omit GLORY_ANDROID_FEATURES.
```

This explicitly disables default features and selects `android-game-activity`.
Never enable it together with `android-native-activity`. The matching Java AAR
and Rust android-activity glue both use GameActivity 4.4.0. The small Java subclass
only loads the native library; it contains no app UI, WebView or second runtime.
Gradle dependency locks and SHA-256 verification metadata are committed. Ordinary
builds verify them; `GLORY_UPDATE_GRADLE_LOCKS=1` is an explicit maintainer update.

The script requires existing tools and puts builds, Java/Gradle homes, temporary
staging and the debug key under `/Volumes/Data/Caches/glory/android`. Override
`ANDROID_HOME`, `ANDROID_NDK_HOME`, `JAVA_HOME` and `GLORY_GRADLE` as needed.
It validates AArch64/JNI entry symbols, ELF LOAD alignment of at least 16 KiB and
APK signing before publishing `package-game-aarch64-debug/glory-counter.apk`.
This checks ELF compatibility; a 4 KiB emulator does not prove 16 KiB runtime
acceptance. Debug signing is not a production release.

GameTextInput sends real UTF-16 selection/composition state to Winit. A bounded
safe-Rust translator produces validated preedit, commit, cancel and surrounding
deletions; it refuses unsupported selection-only moves or autocorrection beyond
the selected range. Android/JVM effects run after releasing the session lock.
The same focused-editor context policy as UIKit keeps password text private and
preserves live marked text across app acknowledgements. NativeActivity does not
acquire GameActivity composition support by sharing this example.

### APK assets through AssetManager

`GLORY_ANDROID_FEATURES=android-assets sh examples/mobile/build-android-game.sh`
selects the facade's `blitz-android-assets`. Gradle stages the same CSS/PNG files
under `assets/glory/` in its data-volume output. The app binds the explicit
`app.asset_manager()` with prefix `glory`, uses base URL `asset://android/` and
leaves the embedded package empty. No fallback masks a failed APK read. This
application-scoped manager remains valid across Activity recreation; it retains
no Activity. NDK calls use safe Rust with no additional JNI bridge.

The existing Loader budgets apply to APK reads and decode admission. Paths decode
UTF-8 once; decoded separators, controls and empty/dot components are refused.
The prefix is separate from URL normalization. Streaming reads enforce declared
and actual byte limits and check cancellation/deadline between bounded chunks;
blocking admission remains held until the actual OS operation ends. Close stops
new work and revokes publication, without claiming to interrupt OS calls.

For actual asset acceptance, use `GLORY_ANDROID_FEATURES=android-assets-probe`.
Only this feature adds a generated Unicode-path test asset and native acceptance
code. Install/launch the APK on an owned `Glory_` emulator, then run:

```sh
python3 examples/mobile/capture-android-assets.py \
  --adb /Volumes/Data/Caches/glory/android/adb36/platform-tools/adb \
  --serial emulator-5580 \
  --package /Volumes/Data/Caches/glory/android/package-game-aarch64-debug \
  --out /Volumes/Data/Caches/glory/android/asset-manager-acceptance
```

The fixture checks real compressed CSS/Unicode reads, PNG, missing and refused
URLs, body limits, decode admission and close cancellation before mounting Counter.
The read-only capture verifies the installed APK against its manifest and checks
actual badge pixels, CSS dimensions/position and PID-specific success logs.
It does not inject input or establish network/permission/device acceptance.
ARM64 Android 16 emulator evidence is under the output directory above.

## iOS

`src/ios.rs`, `ios/Info.plist` and `ios/SceneDelegate.swift` define the UIKit
application. With Xcode 27, UIScene adoption is mandatory: the app-owned Swift
delegate forwards actual scene lifecycle callbacks to Winit, and Shell creates
its surface after scene activation. The pinned Winit fork associates its existing
window with the connected UIWindowScene before making it visible. Inactivation
releases the CPU surface; activation resumes the same semantic document and Loader.
Disconnect detaches the Winit-owned window from its old scene. A later scene
reattaches that same window, resets the viewport and restores a still-live weak
native first responder after activation. Only the current scene may publish
lifecycle events; duplicate and stale callbacks cannot suspend a successor.
This is a single-Glory-window, same-process contract. Process termination does
not persist application state, and independent simultaneous Glory scenes are
not supported by this example.
No additional reactive runtime or unsafe code is introduced in Glory. The Winit
UIKit protocol bridge has the narrowly authorized exception recorded in AGENTS.md.

On a Mac with full Xcode and the simulator Rust standard library:

```sh
export GLORY_BUILD_ROOT=/Volumes/Data/Caches/glory/mobile
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
sh examples/mobile/build-ios.sh
xcrun simctl install <owned-device-udid> "$GLORY_BUILD_ROOT/ios-aarch64-apple-ios-sim-debug/GloryCounter.app"
xcrun simctl launch <owned-device-udid> rs.glory.counter
```

All compiler output, Cargo home, Swift module caches and temporary files stay
under `/Volumes/Data/Caches/glory`; canonical paths reject symlink escapes.
`DEVELOPER_DIR` selects Xcode per process without changing xcode-select. The
application build script compiles its Swift bridge and tracks the source for
relinking. Intel simulators use `GLORY_IOS_TARGET=x86_64-apple-ios`.
`GLORY_IOS_PROFILE=release` selects release output. Packages have a fresh
Info.plist, verified signature and `build.json` hashes of the final signed files.

Device packaging uses `GLORY_IOS_TARGET=aarch64-apple-ios` and requires explicit
`GLORY_SIGN_IDENTITY` and `GLORY_PROVISION_PROFILE`. It validates profile expiry
and application identity, derives entitlements, embeds the profile, verifies the
signature and writes `GloryCounter.ipa` with its Payload directory. This path
has not been signed or installed on a device in this session. The arm64 device
executable has actually linked; that alone does not establish device acceptance.

Apple Silicon/Xcode 27.1/iOS 27 simulator evidence covers actual Counter pixels,
CPU rendering, touch increment, a real software-key `7` changing `1` to `17`,
same-process Home/restore, landscape rotation and retained value/embedded assets.
The 32-point PNG and its safe-area-relative CSS position are checked from raw
screenshots. `capture-ios.py` (Python with Pillow) records a named UI phase and
can require the same label pixels as a previous phase. It does not synthesize
UI input or claim that a screenshot proves accessibility/IME support.

```sh
python3 examples/mobile/capture-ios.py --device <owned-device-udid> \
  --out /Volumes/Data/Caches/glory/ios-acceptance --phase resumed \
  --same-label /Volumes/Data/Caches/glory/ios-acceptance/before.json
```

For the iPhone 17 landscape fixture, add `--safe-left 62 --safe-right 62`.
CoreSimulator uses its OS-managed device directory: this host's CoreSimulator
service rejects writes to the data volume. The helper captures into that owned
device's tmp directory and moves only the resulting evidence to the data volume.
Xcode 27's simulator UI is Device Hub (`Xcode.app/Contents/Applications/DeviceHub.app`).

The UIKit fork now implements the required UITextInput selectors using one
non-displayed UIKit text store per native view. UIKit owns its opaque positions,
ranges and tokenizer; Glory/Blitz still own the visible editor and semantic state.
Committed surrounding text is a UTF-8-boundary-safe excerpt under 4000 bytes,
exported only by the focused editor. Password context is empty. Acknowledgements
and purpose changes preserve active marked text; UTF-16 native selection offsets
become validated UTF-8 preedit offsets. Disable/Clear cancels the old composition.

Real system pinyin key taps now produce marked `nihao`, a `你好` candidate and
exactly one committed `0你好`. Tests also cover Chinese backspace, emoji buffer
insertion/grapheme deletion, deleting preedit back to the committed value, Clear
during composition, refocusing with a fresh context, same-process Home/resume and
continued Chinese input after rotation. Old failing screenshots remain separately
in the evidence directory. Emoji buffer/offset acceptance does not establish
color-emoji painting.

An additional opt-in view makes nonempty replacement/cancellation reproducible:

```sh
GLORY_IOS_FEATURES=ime-probe sh examples/mobile/build-ios.sh
```

It starts with `A中😀Z`. Tap **Select middle**, type `ni`, then delete both preedit
letters: both the visible value and native text context must return to `A中😀Z`
with `中😀` selected (UTF-16 offsets 1..4). Type
`nihao` and choose `你好`: the result must be `A你好Z` with status Idle. **Restore**
returns to the original fixture; **Reset** tests a differing program value.
The ordinary counter build excludes this view. Its feature selection is recorded
in `build.json`. Native IME callbacks invalidate the context acknowledgement
cache so equal-value cancellation can restore the native store and selection.

The app-owned Swift scene delegate observes docked keyboard frames only for its
own key window and updates the root controller's additional safe-area inset.
Winit's existing layout event refreshes the native viewport; no semantic model
translation or keyboard-sized padding is injected into Glory. Floating keyboards
are ignored. `keyboard-probe` places an input near the bottom for real show/hide
acceptance; select it separately from `ime-probe`. Twelve standalone Swift tests
cover clipping, safe areas, undocked frames and the minimum retained viewport.

**Open gaps:** arbitrary native selection gestures, selection rectangles and
unrequested autocorrection outside the application's current selection are not
advertised by this IME bridge. VoiceOver, signed
device installation, App Store icons/distribution and real-device networking/
permissions remain separate work. A simulator pass does not close those items.

### Scene disconnect/reconnect acceptance

`GLORY_IOS_SCENE_MULTI=1 sh examples/mobile/build-ios.sh` builds a separate
simulator-only `-scene-probe` package with bundle ID `rs.glory.counter.sceneprobe`.
Launch it with `SIMCTL_CHILD_GLORY_IOS_SCENE_PROBE=1`. This compile-time fixture
enables multiple UIKit scenes and shows native acceptance controls. Ordinary
packages contain neither these controls nor the additional scene configuration.

On an owned iPad simulator, tap **Keep process alive**, then **Open Glory scene**.
Edit the counter using its real software keyboard, tap **Discard scene**, then
reopen the fixture's anchor and **Open Glory scene**. Discard calls UIKit's actual
scene-session destruction API. The native anchor keeps the process alive while
the Glory scene is absent; it creates no Rust event loop or document. Destroying
the last scene may terminate the process, and the tested iPhone simulator refused
scene-session destruction. Neither outcome is counted as reconnect acceptance.

The iPad Air M4/iOS 27 run reconnected twice with the same PID and native window,
preserved `17`, accepted a software key to produce `177`, and restored `177`
after another disconnect and rotation to landscape. Keyboard restoration and
safe-area insets were observed, and `capture-ios.py --same-label` checked retained
text and CSS/PNG pixels. Evidence: `/Volumes/Data/Caches/glory/scene-final-acceptance/`
and `scene-clean-final-console.log`. Pure host tests cover duplicate/stale callbacks
and 1,000 reconnects; these do not replace the actual UIKit run.

The Windows canonical gate accepts `-Android` to check the adapter and build both
APKs. Add `-AndroidSerial emulator-5582` to install the newly built x86_64 APK on
an explicitly selected, already running owned emulator and run acceptance;
APK builds alone are reported separately from runtime
acceptance. The native adapter check uses `--no-deps` because the current Android
Clippy diagnoses already-const thread locals in core.
