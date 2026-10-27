#!/bin/sh
set -eu
project=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$project/../.." && pwd)
. "$repo/scripts/apple-env.sh"
case "${GLORY_IOS_TARGET:-aarch64-apple-ios-sim}" in
  aarch64-apple-ios-sim) fixture_arch=arm64 ;;
  x86_64-apple-ios) fixture_arch=x86_64 ;;
  *) echo 'The owned document fixture is simulator-only' >&2; exit 1 ;;
esac
fixture_root=$(python3 - "$GLORY_BUILD_ROOT" <<'CHECK'
import pathlib,sys
base=pathlib.Path(sys.argv[1]).resolve()
path=(base/'ios-files-fixture').resolve()
if not path.is_relative_to(base) or path == base:
    raise SystemExit('Fixture build directory escaped the approved cache')
print(path)
CHECK
)
mkdir -p "$fixture_root"
fixture_stage=$(mktemp -d "$fixture_root/stage.XXXXXX")
trap 'rm -rf "$fixture_stage"' EXIT HUP INT TERM
mkdir "$fixture_stage/GloryProvider.app"
cp "$project/ios/files-fixture/Info.plist" "$fixture_stage/GloryProvider.app/Info.plist"
xcrun --sdk iphonesimulator swiftc "$project/ios/files-fixture/main.swift" \
  -target "$fixture_arch-apple-ios15.0-simulator" \
  -sdk "$(xcrun --sdk iphonesimulator --show-sdk-path)" -warnings-as-errors \
  -module-cache-path "$fixture_root/SwiftModules" \
  -o "$fixture_stage/GloryProvider.app/GloryDocuments"
codesign --force --sign - "$fixture_stage/GloryProvider.app"
codesign --verify --strict "$fixture_stage/GloryProvider.app"
plutil -lint "$fixture_stage/GloryProvider.app/Info.plist"
# Only an owned build output is replaced. Never mutate an installed app/data.
rm -rf "$fixture_root/GloryProvider.app"
mv "$fixture_stage/GloryProvider.app" "$fixture_root/GloryProvider.app"
printf 'Owned document fixture: %s\n' "$fixture_root/GloryProvider.app"
