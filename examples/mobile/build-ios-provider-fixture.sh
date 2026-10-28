#!/bin/sh
set -eu
project=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$project/../.." && pwd)
. "$repo/scripts/apple-env.sh"
case "${GLORY_IOS_TARGET:-aarch64-apple-ios-sim}" in
  aarch64-apple-ios-sim) fixture_arch=arm64 ;;
  x86_64-apple-ios) fixture_arch=x86_64 ;;
  *) echo 'The owned provider fixture is simulator-only' >&2; exit 1 ;;
esac
fixture_root=$(python3 - "$GLORY_BUILD_ROOT" "${GLORY_PROVIDER_MODE:-enumerating}" <<'CHECK'
import pathlib,sys
base=pathlib.Path(sys.argv[1]).resolve()
path=(base/'ios-provider-fixture'/sys.argv[2]).resolve()
if not path.is_relative_to(base) or path == base:
    raise SystemExit('Fixture build directory escaped the approved cache')
print(path)
CHECK
)
mkdir -p "$fixture_root"
fixture_stage=$(mktemp -d "$fixture_root/stage.XXXXXX")
trap 'rm -rf "$fixture_stage"' EXIT HUP INT TERM
fixture_source="$project/ios/provider-fixture"
case "${GLORY_PROVIDER_MODE:-enumerating}" in
  enumerating) provider_source=EnumeratingProvider.swift; provider_principal=GloryEnumeratingProvider ;;
  replicated) provider_source=Provider.swift; provider_principal=GloryReplicatedProvider ;;
  *) echo 'Invalid GLORY_PROVIDER_MODE' >&2; exit 1 ;;
esac
fixture_app="$fixture_stage/GloryHydration.app"
fixture_extension="$fixture_app/PlugIns/GloryHydrationProvider.appex"
mkdir -p "$fixture_extension"
cp "$fixture_source/Host.plist" "$fixture_app/Info.plist"
cp "$fixture_source/Extension.plist" "$fixture_extension/Info.plist"
/usr/libexec/PlistBuddy -c "Set :NSExtension:NSExtensionPrincipalClass $provider_principal" "$fixture_extension/Info.plist"
if [ "$provider_principal" = GloryEnumeratingProvider ]; then
  for provider_plist in "$fixture_app/Info.plist" "$fixture_extension/Info.plist"; do
    /usr/libexec/PlistBuddy -c 'Set :CFBundleDisplayName Glory Downloads' "$provider_plist"
    /usr/libexec/PlistBuddy -c 'Set :CFBundleName Glory Downloads' "$provider_plist"
  done
  /usr/libexec/PlistBuddy -c 'Set :CFBundleIdentifier rs.glory.hydration.enumerating' "$fixture_app/Info.plist"
  /usr/libexec/PlistBuddy -c 'Set :CFBundleIdentifier rs.glory.hydration.enumerating.provider' "$fixture_extension/Info.plist"
  /usr/libexec/PlistBuddy -c 'Set :NSExtension:NSExtensionFileProviderDocumentGroup group.rs.glory.hydration.enumerating' "$fixture_extension/Info.plist"
  provider_flags=-DGLORY_ENUMERATING
else
  provider_flags=-DGLORY_REPLICATED
fi
cp "$fixture_source/Entitlements.plist" "$fixture_stage/entitlements.plist"
if [ "$provider_principal" = GloryEnumeratingProvider ]; then
  /usr/libexec/PlistBuddy -c 'Set :com.apple.security.application-groups:0 group.rs.glory.hydration.enumerating' "$fixture_stage/entitlements.plist"
fi
fixture_sdk=$(xcrun --sdk iphonesimulator --show-sdk-path)
# Simulator entitlements belong in the executable as well as its signature.
xcrun --sdk iphonesimulator swiftc $provider_flags "$fixture_source/$provider_source" "$fixture_source/Model.swift" "$fixture_source/Download.swift" "$fixture_source/Evidence.swift" \
  -target "$fixture_arch-apple-ios16.0-simulator" -sdk "$fixture_sdk" \
  -warnings-as-errors -application-extension -parse-as-library -Xlinker -e -Xlinker _NSExtensionMain -module-name GloryHydrationProvider \
  -Xlinker -sectcreate -Xlinker __TEXT -Xlinker __entitlements -Xlinker "$fixture_stage/entitlements.plist" \
  -module-cache-path "$fixture_root/SwiftModules" -o "$fixture_extension/GloryHydrationProvider"
xcrun --sdk iphonesimulator swiftc $provider_flags "$fixture_source/main.swift" \
  -target "$fixture_arch-apple-ios16.0-simulator" -sdk "$fixture_sdk" -warnings-as-errors \
  -Xlinker -sectcreate -Xlinker __TEXT -Xlinker __entitlements -Xlinker "$fixture_stage/entitlements.plist" \
  -module-cache-path "$fixture_root/SwiftModules" -o "$fixture_app/GloryHydrationHost"
codesign --force --sign - --entitlements "$fixture_stage/entitlements.plist" "$fixture_extension"
codesign --force --sign - --entitlements "$fixture_stage/entitlements.plist" "$fixture_app"
codesign --verify --deep --strict "$fixture_app"
plutil -lint "$fixture_app/Info.plist" "$fixture_extension/Info.plist"
# Only an owned build output is replaced. Never mutate an installed app/data.
rm -rf "$fixture_root/GloryHydration.app"
mv "$fixture_stage/GloryHydration.app" "$fixture_root/GloryHydration.app"
printf 'Owned provider fixture: %s\n' "$fixture_root/GloryHydration.app"
