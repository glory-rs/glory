#!/bin/sh
# WSL validation with an explicitly verified D:-backed build/cache filesystem.
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
glory_build_root=$(realpath -e "${GLORY_BUILD_ROOT:?Set GLORY_BUILD_ROOT to an existing directory under /mnt/d/BuildCache}")
case "$glory_build_root" in /mnt/d/BuildCache/*) ;; *) echo 'Build root must resolve under /mnt/d/BuildCache' >&2; exit 1 ;; esac
glory_device=$(findmnt -n -o SOURCE --target "$glory_build_root")
glory_fs=$(findmnt -n -o FSTYPE --target "$glory_build_root")
case "$glory_fs:$glory_device" in
    '9p://localhost/D$'|'drvfs:D:'|'9p:D:') ;;
    ext4:/dev/loop*)
        glory_loop=${glory_device##*/}
        glory_backing=$(realpath -e "$(cat "/sys/class/block/$glory_loop/loop/backing_file")")
        case "$glory_backing" in /mnt/d/BuildCache/*) ;; *) echo 'Loop backing file is outside D: BuildCache' >&2; exit 1 ;; esac
        glory_backing_device=$(findmnt -n -o SOURCE --target "$glory_backing")
        case "$glory_backing_device" in '//localhost/D$'|'D:') ;; *) echo 'Loop backing file is not on verified D:' >&2; exit 1 ;; esac
        printf 'Verified %s backing file: %s\n' "$glory_device" "$glory_backing"
        ;;
    *) echo "Unverified build filesystem: $glory_fs $glory_device" >&2; exit 1 ;;
esac
glory_artifacts=$(realpath -m "${GLORY_ARTIFACT_DIR:-$glory_build_root/artifacts}")
case "$glory_artifacts" in /mnt/d/BuildCache/*) ;; *) echo 'Artifacts must resolve under /mnt/d/BuildCache' >&2; exit 1 ;; esac
export CARGO_HOME="$glory_build_root/cargo"
export CARGO_TARGET_DIR="$glory_build_root/target"
export TMPDIR="$glory_build_root/tmp"
export TEMP="$TMPDIR"
export TMP="$TMPDIR"
export XDG_CACHE_HOME="$glory_build_root/cache"
export PYTHONPYCACHEPREFIX="$glory_build_root/pycache"
mkdir -p "$CARGO_HOME" "$CARGO_TARGET_DIR" "$TMPDIR" "$XDG_CACHE_HOME" "$glory_artifacts"
cd "$glory_workspace"
python3 scripts/library_lockfiles.py
cargo metadata --no-deps --format-version 1 > "$glory_artifacts/metadata-linux.json"
python3 -c 'import json,os,sys; p=json.load(open(sys.argv[1]))["target_directory"]; assert os.path.realpath(p)==os.path.realpath(os.environ["CARGO_TARGET_DIR"]),p; print("Verified target:",p)' "$glory_artifacts/metadata-linux.json"
python3 scripts/check-engine-forks.py --test
cargo fmt --all -- --check
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-features --all-targets -- -D warnings
cargo clippy --locked -p glory-apace --no-default-features --all-targets -- -D warnings
cargo clippy --locked -p glory-apace --no-default-features --features paint --all-targets -- -D warnings
if "$glory_paint"; then
    cargo run --locked -p glory-apace --example raster --features raster -- "$glory_artifacts/apace-paint-linux.png"
fi
if "$glory_window"; then
    for GLORY_WINDOW_BACKEND in x11 wayland; do
        export GLORY_WINDOW_BACKEND
        printf 'Native window acceptance: %s\n' "$GLORY_WINDOW_BACKEND"
        cargo test --locked -p glory-apace --features shell --lib real_windows -- --ignored --nocapture --test-threads=1
        cargo test --locked -p glory-apace --features accesskit --lib real_windows -- --ignored --nocapture --test-threads=1
    done
fi
