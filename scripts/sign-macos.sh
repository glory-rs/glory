#!/bin/sh
# Explicit release credentials are owned by the application, never inferred.
set -eu
repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$repo/scripts/apple-env.sh"
[ "$#" = 3 ] || { echo 'Usage: sign-macos.sh APP DEVELOPER_ID_IDENTITY NOTARY_KEYCHAIN_PROFILE' >&2; exit 1; }
app=$(python3 - "$1" <<'PY'
import pathlib,sys
p=pathlib.Path(sys.argv[1]).resolve()
if not p.is_relative_to('/Volumes/Data/Caches/glory') or p.suffix != '.app' or not (p/'Contents/Info.plist').is_file():
    raise SystemExit('Expected a built application under the Apple cache root')
print(p)
PY
)
[ -n "$2" ] && [ "$2" != - ] && [ -n "$3" ] || { echo 'Developer ID identity and saved notary profile are required' >&2; exit 1; }
package=$(dirname -- "$app")
[ -f "$package/glory-build.json" ] || { echo 'CLI package manifest is required' >&2; exit 1; }
stage=$(mktemp -d "$(dirname -- "$package")/.glory-notary.XXXXXX")
trap 'status=$?; if [ "$status" -eq 0 ] && [ ! -e "$stage/previous" ]; then rm -rf "$stage"; else echo "Signing did not publish; diagnostics and staged files retained at $stage" >&2; fi' EXIT
trap 'exit 1' HUP INT TERM
ditto "$package" "$stage/package"
app="$stage/package/$(basename -- "$app")"
codesign --force --options runtime --timestamp --sign "$2" "$app"
codesign --verify --strict "$app"
archive="$stage/application.zip"
ditto -c -k --keepParent "$app" "$archive"
xcrun notarytool submit "$archive" --keychain-profile "$3" --wait --output-format json > "$stage/package/notary-result.json"
python3 - "$stage/package/notary-result.json" <<'PY'
import json,sys
result=json.load(open(sys.argv[1]))
if result.get('status') != 'Accepted':
    raise SystemExit('Notarization did not return Accepted; inspect notary-result.json')
PY
xcrun stapler staple "$app"
xcrun stapler validate "$app"
spctl --assess --type execute --verbose "$app"
# Refresh the CLI manifest after signature and staple change bytes.
python3 - "$stage/package" <<'PY'
import hashlib,json,pathlib,sys
root=pathlib.Path(sys.argv[1]); path=root/'glory-build.json'
manifest=json.loads(path.read_text())
manifest['files']=[dict(path=p.relative_to(root).as_posix(),bytes=p.stat().st_size,
    sha256=hashlib.sha256(p.read_bytes()).hexdigest())
    for p in sorted(root.rglob('*')) if p.is_file() and p != path]
manifest['apple_distribution']='developer-id-notarized'
path.write_text(json.dumps(manifest,indent=2)+'\n')
PY
# Publish only after every release check and final hash succeeded.
mv "$package" "$stage/previous"
if ! mv "$stage/package" "$package"; then
    mv "$stage/previous" "$package" || { echo "Recover previous package at $stage/previous" >&2; exit 1; }
    exit 1
fi
rm -rf "$stage/previous"
