#!/bin/sh
# Source this file before invoking Cargo or Apple's SDK tools.
[ "$(uname -s)" = Darwin ] || { echo 'Apple builds require macOS' >&2; exit 1; }
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
[ -d "$DEVELOPER_DIR/Platforms" ] || { echo 'Set DEVELOPER_DIR to a full Xcode installation' >&2; exit 1; }
export GLORY_BUILD_ROOT=${GLORY_BUILD_ROOT:-/Volumes/Data/Caches/glory/semantic-ui}
# Resolve existing ancestors before mkdir; a symlink must not create files elsewhere.
GLORY_BUILD_ROOT=$(python3 - <<'PY'
import os,pathlib
p=pathlib.Path(os.environ['GLORY_BUILD_ROOT'])
base=pathlib.Path('/Volumes/Data/Caches/glory')
if not p.is_absolute() or '..' in p.parts or p == base or not p.is_relative_to(base):
    raise SystemExit('Apple build root must be under /Volumes/Data/Caches/glory/')
p=p.resolve()
if not p.is_relative_to(base) or p == base:
    raise SystemExit('Apple build root escaped the data-volume cache')
print(p)
PY
) || exit 1
export GLORY_BUILD_ROOT
export CARGO_HOME="$GLORY_BUILD_ROOT/cargo" CARGO_TARGET_DIR="$GLORY_BUILD_ROOT/target"
export TMPDIR="$GLORY_BUILD_ROOT/tmp" TEMP="$GLORY_BUILD_ROOT/tmp" TMP="$GLORY_BUILD_ROOT/tmp"
export PYTHONPYCACHEPREFIX="$GLORY_BUILD_ROOT/pycache"
python3 - <<'PY'
import os,pathlib
base=pathlib.Path('/Volumes/Data/Caches/glory')
for key in ('CARGO_HOME','CARGO_TARGET_DIR','TMPDIR','PYTHONPYCACHEPREFIX'):
    p=pathlib.Path(os.environ[key]).resolve()
    if not p.is_relative_to(base):
        raise SystemExit(f'{key} escaped the data-volume cache')
    p.mkdir(parents=True,exist_ok=True)
PY
# Neither xcode-select nor the user's global SDK environment is changed.
xcrun --find clang >/dev/null
