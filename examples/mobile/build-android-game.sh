#!/bin/sh
# GameActivity's Java AAR and Rust glue must use the same pinned release.
set -eu
repo=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
export GLORY_BUILD_ROOT=${GLORY_BUILD_ROOT:-/Volumes/Data/Caches/glory/android}
. "$repo/scripts/apple-env.sh"
export ANDROID_HOME=${ANDROID_HOME:-$GLORY_BUILD_ROOT/sdk}
export ANDROID_NDK_HOME=${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/android-ndk-r27d}
export JAVA_HOME=${JAVA_HOME:-/Applications/Android Studio.app/Contents/jbr/Contents/Home}
export GRADLE_USER_HOME="$GLORY_BUILD_ROOT/gradle-home"
export ANDROID_USER_HOME="$GLORY_BUILD_ROOT/android-home"
export JAVA_TOOL_OPTIONS="-Djava.io.tmpdir=$TMPDIR -Duser.home=$GLORY_BUILD_ROOT/java-home"
gradle=${GLORY_GRADLE:-$GLORY_BUILD_ROOT/gradle/gradle-9.1.0/bin/gradle}
compiler="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin"
[ -x "$gradle" ] && [ -x "$compiler/aarch64-linux-android28-clang" ] && [ -x "$JAVA_HOME/bin/java" ] || {
    echo 'Set GLORY_GRADLE, ANDROID_NDK_HOME and JAVA_HOME to installed tools' >&2; exit 1;
}
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$compiler/aarch64-linux-android28-clang"
export CC_aarch64_linux_android="$compiler/aarch64-linux-android28-clang"
export CXX_aarch64_linux_android="$compiler/aarch64-linux-android28-clang++"
export AR_aarch64_linux_android="$compiler/llvm-ar"
features=android-game-activity
if [ -n "${GLORY_ANDROID_FEATURES:-}" ]; then features="$features,$GLORY_ANDROID_FEATURES"; fi
cd "$repo"
cargo rustc --locked --manifest-path examples/mobile/Cargo.toml --lib --target aarch64-linux-android --no-default-features --features "$features" -- -C link-arg=-Wl,-z,max-page-size=16384
library="$CARGO_TARGET_DIR/aarch64-linux-android/debug/libglory_mobile_counter.so"
"$compiler/llvm-readelf" -h "$library" | grep -q 'AArch64'
"$compiler/llvm-readelf" -l "$library" > "$GLORY_BUILD_ROOT/game-program-headers.txt"
python3 - "$GLORY_BUILD_ROOT/game-program-headers.txt" <<'PY'
import pathlib,sys
loads=[line.split() for line in pathlib.Path(sys.argv[1]).read_text().splitlines() if line.strip().startswith('LOAD ')]
assert loads and all(int(line[-1],16)>=16384 for line in loads), 'ELF must support 16 KiB page alignment'
PY
"$compiler/llvm-nm" -D --defined-only "$library" > "$GLORY_BUILD_ROOT/game-symbols.txt"
for symbol in android_main GameActivity_onCreate Java_com_google_androidgamesdk_GameActivity_initializeNativeCode; do
    grep -q " $symbol\$" "$GLORY_BUILD_ROOT/game-symbols.txt" || { echo "Missing entry: $symbol" >&2; exit 1; }
done
mkdir -p "$ANDROID_USER_HOME" "$GLORY_BUILD_ROOT/java-home"
export GLORY_ANDROID_DEBUG_KEY="$ANDROID_USER_HOME/debug.keystore"
if [ ! -f "$GLORY_ANDROID_DEBUG_KEY" ]; then
    "$JAVA_HOME/bin/keytool" -genkeypair -keystore "$GLORY_ANDROID_DEBUG_KEY" -storepass android -alias androiddebugkey -keypass android -dname 'CN=Glory Debug' -keyalg RSA -validity 10000
fi
stage=$(mktemp -d "$GLORY_BUILD_ROOT/android-game-stage.XXXXXX")
trap '[ -e "$stage/previous" ] || rm -rf "$stage"' EXIT HUP INT TERM
mkdir -p "$stage/lib/arm64-v8a"
cp "$library" "$stage/lib/arm64-v8a/"
"$compiler/llvm-strip" --strip-unneeded "$stage/lib/arm64-v8a/libglory_mobile_counter.so"
export GLORY_ANDROID_LIBS="$stage/lib" GLORY_ANDROID_GRADLE_OUTPUT="$GLORY_BUILD_ROOT/gradle-build"
set -- assembleDebug
if [ "${GLORY_UPDATE_GRADLE_LOCKS:-0}" = 1 ]; then set -- "$@" --write-locks --write-verification-metadata sha256; fi
"$gradle" --no-daemon --project-dir examples/mobile/android-game --project-cache-dir "$GLORY_BUILD_ROOT/gradle-project" "$@"
cp "$GLORY_ANDROID_GRADLE_OUTPUT/outputs/apk/debug/GloryGameActivity-debug.apk" "$stage/glory-counter.apk"
"$ANDROID_HOME/build-tools/36.0.0/apksigner" verify "$stage/glory-counter.apk"
python3 - "$stage" "$features" <<'PY'
import hashlib,json,pathlib,sys
root=pathlib.Path(sys.argv[1]); apk=root/'glory-counter.apk'
(root/'build.json').write_text(json.dumps(dict(target='aarch64-linux-android',abi='arm64-v8a',
    activity='GameActivity',game_activity='4.4.0',signing='debug-only',features=sys.argv[2].split(','),
    sha256=hashlib.sha256(apk.read_bytes()).hexdigest()),indent=2)+'\n')
PY
rm -rf "$stage/lib"
destination="$GLORY_BUILD_ROOT/package-game-aarch64-debug"
if [ -e "$destination" ]; then mv "$destination" "$stage/previous"; fi
if ! mv "$stage" "$destination"; then
    [ ! -e "$stage/previous" ] || mv "$stage/previous" "$destination"
    exit 1
fi
rm -rf "$destination/previous"
echo "GameActivity APK: $destination/glory-counter.apk"
