#!/bin/sh
set -eu
repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$repo/scripts/apple-env.sh"
cd "$repo"
cargo test --locked -p glory-apace --features clipboard --lib --no-run --message-format=json > "$GLORY_BUILD_ROOT/clipboard-test-build.jsonl"
binary=$(python3 - "$GLORY_BUILD_ROOT/clipboard-test-build.jsonl" <<'PY'
import json,sys
items=[json.loads(line) for line in open(sys.argv[1])]
bins=[x['executable'] for x in items if x.get('reason')=='compiler-artifact' and x.get('profile',{}).get('test') and x.get('target',{}).get('name')=='glory_apace' and x.get('executable')]
assert len(bins)==1,bins
print(bins[0])
PY
)
xcrun swiftc -module-cache-path "$GLORY_BUILD_ROOT/SwiftHostModules" \
    scripts/macos-clipboard-guard.swift -o "$GLORY_BUILD_ROOT/clipboard-guard"
"$GLORY_BUILD_ROOT/clipboard-guard" "$binary"
