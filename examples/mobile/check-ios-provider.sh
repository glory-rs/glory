#!/bin/sh
set -eu
project=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
. "$project/../../scripts/apple-env.sh"
xcrun --sdk macosx swiftc "$project/ios/provider-fixture/Model.swift" \
  "$project/ios/provider-fixture/Download.swift" "$project/ios/provider-tests/main.swift" \
  -warnings-as-errors -module-cache-path "$GLORY_BUILD_ROOT/SwiftProviderTests" \
  -o "$GLORY_BUILD_ROOT/provider-tests"
"$GLORY_BUILD_ROOT/provider-tests"
