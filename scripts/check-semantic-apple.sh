#!/bin/sh
set -eu
repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$repo/scripts/apple-env.sh"
cd "$repo"
cargo metadata --locked --no-deps --format-version 1 > "$GLORY_BUILD_ROOT/metadata.json"
python3 - "$GLORY_BUILD_ROOT/metadata.json" <<'PY'
import json,os,pathlib,sys
assert pathlib.Path(json.load(open(sys.argv[1]))['target_directory']).resolve() == pathlib.Path(os.environ['CARGO_TARGET_DIR']).resolve()
PY
cargo fmt --all -- --check
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-features --all-targets -- -D warnings
cargo fmt --manifest-path tools/cli/Cargo.toml --all -- --check
cargo test --locked --manifest-path tools/cli/Cargo.toml
cargo clippy --locked --manifest-path tools/cli/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path examples/mobile/Cargo.toml --all -- --check
xcrun swiftc -module-cache-path "$GLORY_BUILD_ROOT/SwiftHostModules" \
    examples/mobile/ios/SceneDelegate.swift examples/mobile/ios/tests/main.swift \
    -o "$GLORY_BUILD_ROOT/keyboard-geometry-tests"
"$GLORY_BUILD_ROOT/keyboard-geometry-tests"
xcrun swiftc -module-cache-path "$GLORY_BUILD_ROOT/SwiftHostModules" \
    examples/mobile/ios/Permissions.swift examples/mobile/ios/permissions-tests/main.swift \
    -o "$GLORY_BUILD_ROOT/permission-ledger-tests"
"$GLORY_BUILD_ROOT/permission-ledger-tests"
for mode in "$@"; do
    case "$mode" in
        --window) GLORY_MACOS_WINDOW=1 cargo test --locked -p glory-blitz --features shell,accesskit --test macos_window ;;
        --ios) GLORY_BUILD_ROOT="$GLORY_BUILD_ROOT/ios" sh examples/mobile/build-ios.sh ;;
        --forks) "${GLORY_PYTHON:-python3}" scripts/check-engine-forks.py --test ;;
        *) echo "Unknown check: $mode" >&2; exit 1 ;;
    esac
done
