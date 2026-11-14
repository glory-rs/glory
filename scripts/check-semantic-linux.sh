#!/bin/sh
# Linux validation using the caller Cargo configuration.
set -eu
ulimit -c 0
glory_paint=false
glory_window=false
for glory_option in "$@"; do
    case "$glory_option" in
        --paint) glory_paint=true ;;
        --window) glory_window=true ;;
        *) echo 'Usage: check-semantic-linux.sh [--paint] [--window]' >&2; exit 2 ;;
    esac
done
glory_script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
glory_workspace=$(dirname -- "$glory_script_dir")
cd "$glory_workspace"
glory_target=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
glory_build_root=${GLORY_BUILD_ROOT:-$glory_target}
glory_artifacts=$(realpath -m "${GLORY_ARTIFACT_DIR:-$glory_build_root/verification/linux}")
mkdir -p "$glory_artifacts"
python3 scripts/library_lockfiles.py
python3 scripts/check-native-renderers.py
cargo metadata --no-deps --format-version 1 > "$glory_artifacts/metadata-linux.json"
python3 -c 'import json,sys; print("Verified target:",json.load(open(sys.argv[1]))["target_directory"])' "$glory_artifacts/metadata-linux.json"
python3 scripts/check-engine-forks.py --test
cargo fmt --all -- --check
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-features --all-targets -- -D warnings
cargo clippy --locked -p glory-apace --no-default-features --all-targets -- -D warnings
cargo clippy --locked -p glory-apace --no-default-features --features paint --all-targets -- -D warnings
cargo test --locked -p glory-apace --no-default-features --features shell --lib
cargo clippy --locked -p glory-apace --no-default-features --features accesskit --all-targets -- -D warnings
cargo clippy --locked -p glory-apace --no-default-features --features accesskit-cpu --all-targets -- -D warnings
if "$glory_paint"; then
    cargo run --locked -p glory-apace --example raster --features raster -- "$glory_artifacts/apace-paint-linux.png"
fi
if "$glory_window"; then
    for GLORY_WINDOW_BACKEND in x11 wayland; do
        export GLORY_WINDOW_BACKEND
        printf 'Native window acceptance: %s\n' "$GLORY_WINDOW_BACKEND"
        cargo test --locked -p glory-apace --features shell --lib real_windows -- --ignored --nocapture --test-threads=1
        cargo test --locked -p glory-apace --features accesskit --lib real_windows -- --ignored --nocapture --test-threads=1
        cargo test --locked -p glory-apace --no-default-features --features shell-cpu --lib real_windows -- --ignored --nocapture --test-threads=1
        cargo test --locked -p glory-apace --no-default-features --features accesskit-cpu --lib real_windows -- --ignored --nocapture --test-threads=1
    done
fi
