#!/bin/sh
set -eu
# A simulator package is deliberately separate from signed device distribution.
[ "$(uname -s)" = Darwin ] || { echo 'iOS requires a macOS host with Xcode; Windows cross-checks are not iOS acceptance.' >&2; exit 1; }
project=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
target=${GLORY_IOS_TARGET:-aarch64-apple-ios-sim}
case "$target" in aarch64-apple-ios-sim|x86_64-apple-ios) ;; *) echo 'This script builds simulator apps only; device distribution needs explicit signing/provisioning.' >&2; exit 1;; esac
: "${GLORY_BUILD_ROOT:?Set GLORY_BUILD_ROOT to a D-backed mounted directory}"
build=$(python3 -c 'import os; print(os.path.realpath(os.environ["GLORY_BUILD_ROOT"]))')
# This repository requires D: build storage even on a remote Apple host.
# Accept a Windows administrative D$ SMB share, not an arbitrary local volume.
mounts=$(mount -t smbfs)
if ! printf '%s\n' "$mounts" | GLORY_VERIFIED_ROOT="$build" python3 -c '
import os,re,sys
root=os.environ["GLORY_VERIFIED_ROOT"]
for line in sys.stdin:
    match=re.match(r"//[^ ]+/[dD]\$ on (.+) \(smbfs[,)]",line)
    if match and root.startswith(match[1].rstrip("/")+"/BuildCache/"):
        break
else:
    raise SystemExit("Build root must be inside BuildCache on a verified Windows D$ SMB mount")
'; then exit 1; fi
export CARGO_HOME="$build/cargo" CARGO_TARGET_DIR="$build/target"
export TMPDIR="$build/tmp" TEMP="$build/tmp" TMP="$build/tmp" PYTHONPYCACHEPREFIX="$build/pycache"
mkdir -p "$CARGO_HOME" "$CARGO_TARGET_DIR" "$TMPDIR"
export SDKROOT=$(xcrun --sdk iphonesimulator --show-sdk-path)
export IPHONEOS_DEPLOYMENT_TARGET=15.0
cd "$project"
cargo metadata --locked --no-deps --format-version 1 > "$build/metadata.json"
python3 - "$build/metadata.json" "$CARGO_TARGET_DIR" <<'PY'
import json,pathlib,sys
actual=pathlib.Path(json.load(open(sys.argv[1]))["target_directory"]).resolve()
assert actual == pathlib.Path(sys.argv[2]).resolve(), f"Unexpected Cargo target: {actual}"
PY
cargo build --locked --bin counter-ios --target "$target"
app="$build/ios-$target/GloryCounter.app"
mkdir -p "$app"
cp "$CARGO_TARGET_DIR/$target/debug/counter-ios" "$app/counter-ios"
cp "$project/ios/Info.plist" "$app/Info.plist"
plutil -lint "$app/Info.plist"
codesign --force --sign - "$app"
codesign --verify "$app"
printf 'iOS simulator app: %s\nInstall: xcrun simctl install booted "%s"\nLaunch: xcrun simctl launch booted rs.glory.counter\n' "$app" "$app"
