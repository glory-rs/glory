#!/bin/sh
set -eu
# A simulator package is deliberately separate from signed device distribution.
[ "$(uname -s)" = Darwin ] || { echo 'iOS requires a macOS host with Xcode; Windows cross-checks are not iOS acceptance.' >&2; exit 1; }
project=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$project/../.." && pwd)
. "$repo/scripts/apple-env.sh"
target=${GLORY_IOS_TARGET:-aarch64-apple-ios-sim}
case "$target" in
    aarch64-apple-ios-sim|x86_64-apple-ios) sdk=iphonesimulator; platform=iPhoneSimulator ;;
    aarch64-apple-ios) sdk=iphoneos; platform=iPhoneOS
        : "${GLORY_SIGN_IDENTITY:?Device builds require an explicit signing identity}"
        : "${GLORY_PROVISION_PROFILE:?Device builds require a provisioning profile}" ;;
    *) echo 'Unsupported iOS target' >&2; exit 1 ;;
esac
profile=${GLORY_IOS_PROFILE:-debug}
case "$profile" in debug) release= ;; release) release=--release ;; *) echo 'Invalid profile' >&2; exit 1;; esac
scene_suffix=
case "${GLORY_IOS_SCENE_MULTI:-0}" in
    0) ;;
    1) [ "$sdk" = iphonesimulator ] || { echo 'Scene destruction fixture is simulator-only' >&2; exit 1; }
       scene_suffix=-scene-probe ;;
    *) echo 'Invalid GLORY_IOS_SCENE_MULTI' >&2; exit 1 ;;
esac
export SDKROOT=$(xcrun --sdk "$sdk" --show-sdk-path)
export IPHONEOS_DEPLOYMENT_TARGET=15.0
build="$GLORY_BUILD_ROOT"
cd "$project"
if [ -n "${GLORY_IOS_FEATURES:-}" ]; then set -- --features "$GLORY_IOS_FEATURES"; else set --; fi
cargo metadata --locked --filter-platform "$target" --format-version 1 "$@" > "$build/metadata.json"
python3 - "$build/metadata.json" "$CARGO_TARGET_DIR" <<'PY'
import json,pathlib,sys
actual=pathlib.Path(json.load(open(sys.argv[1]))["target_directory"]).resolve()
assert actual == pathlib.Path(sys.argv[2]).resolve(), f"Unexpected Cargo target: {actual}"
PY
cargo build --locked --bin counter-ios --target "$target" $release "$@"
# Construct a fresh bundle; obsolete signature/resources cannot survive a rebuild.
package="$build/ios-$target-$profile$scene_suffix"
stage=$(mktemp -d "$build/ios-stage.XXXXXX")
trap '[ -e "$stage/previous" ] || rm -rf "$stage"' EXIT HUP INT TERM
app="$stage/Payload/GloryCounter.app"
mkdir -p "$app"
cp "$CARGO_TARGET_DIR/$target/$profile/counter-ios" "$app/counter-ios"
cp "$project/ios/Info.plist" "$app/Info.plist"
python3 - "$build/metadata.json" "$app/Info.plist" <<'PY'
import json,plistlib,sys
m=json.load(open(sys.argv[1])); ids={p['id'] for p in m['packages'] if p['name']=='glory-apace'}
if any(n['id'] in ids and 'permissions' in n['features'] for n in m['resolve']['nodes']):
    with open(sys.argv[2],'rb') as file: info=plistlib.load(file)
    info.update(NSCameraUsageDescription='Glory permission acceptance checks camera authorization without capturing images.',
        NSMicrophoneUsageDescription='Glory permission acceptance checks microphone authorization without recording audio.',
        NSPhotoLibraryUsageDescription='Glory permission acceptance checks photo-library authorization without reading your photos.',
        NSPhotoLibraryAddUsageDescription='Glory permission acceptance checks photo-add authorization without saving media.',
        NSLocationWhenInUseUsageDescription='Glory permission acceptance checks foreground location authorization without requesting coordinates.')
    with open(sys.argv[2],'wb') as file: plistlib.dump(info,file)
PY
if [ -n "$scene_suffix" ]; then
    /usr/libexec/PlistBuddy -c 'Set :UIApplicationSceneManifest:UIApplicationSupportsMultipleScenes true' "$app/Info.plist"
    /usr/libexec/PlistBuddy -c 'Set :CFBundleIdentifier rs.glory.counter.sceneprobe' "$app/Info.plist"
    /usr/libexec/PlistBuddy -c 'Set :CFBundleDisplayName Glory Scene Probe' "$app/Info.plist"
fi
/usr/libexec/PlistBuddy -c "Add :CFBundleSupportedPlatforms array" "$app/Info.plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleSupportedPlatforms:0 string $platform" "$app/Info.plist"
/usr/libexec/PlistBuddy -c "Add :DTPlatformName string $sdk" "$app/Info.plist"
if [ "$sdk" = iphoneos ]; then
    # Provisioning is supplied by the application owner. Validate its identity,
    # expiry and application entitlement before using it for a device signature.
    security cms -D -i "$GLORY_PROVISION_PROFILE" > "$stage/profile.plist"
    python3 "$repo/scripts/apple-provision.py" "$stage/profile.plist" rs.glory.counter "$stage/entitlements.plist"
    cp "$GLORY_PROVISION_PROFILE" "$app/embedded.mobileprovision"
    codesign --force --sign "$GLORY_SIGN_IDENTITY" --entitlements "$stage/entitlements.plist" "$app"
else
    codesign --force --sign - "$app"
fi
plutil -lint "$app/Info.plist"
codesign --verify --strict "$app"
mkdir -p "$stage/package"
mv "$app" "$stage/package/GloryCounter.app"
if [ "$sdk" = iphoneos ]; then
    mkdir -p "$stage/ipa/Payload"
    ditto "$stage/package/GloryCounter.app" "$stage/ipa/Payload/GloryCounter.app"
    (cd "$stage/ipa" && ditto -c -k --keepParent Payload "$stage/package/GloryCounter.ipa")
fi
python3 "$repo/scripts/apple-manifest.py" "$stage/package" "$target" "$profile"
# Publish the app, optional IPA and matching hashes as one directory. A failed
# signing/archive/hash step leaves the previous complete package untouched.
if [ -e "$package" ]; then mv "$package" "$stage/previous"; fi
if ! mv "$stage/package" "$package"; then
    if [ -e "$stage/previous" ]; then
        mv "$stage/previous" "$package" || { echo "Recover previous package at $stage/previous" >&2; exit 1; }
    fi
    exit 1
fi
rm -rf "$stage/previous"
printf 'iOS app: %s\n' "$package/GloryCounter.app"
