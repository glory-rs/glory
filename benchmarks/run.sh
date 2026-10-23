#!/usr/bin/env bash
# Delegate to the Windows runner with verified D: build storage.
#   ./run.sh glory | leptos | dioxus
set -euo pipefail

app="${1:-}"
case "$app" in
  glory|leptos|dioxus) ;;
  *) echo "usage: $0 {glory|leptos|dioxus}" >&2; exit 1 ;;
esac

entry="$(cd "$(dirname "$0")" && pwd)/run.ps1"
if command -v wslpath >/dev/null; then entry=$(wslpath -w "$entry");
elif command -v cygpath >/dev/null; then entry=$(cygpath -w "$entry"); fi
exec pwsh.exe -NoProfile -File "$entry" -App "$app"
