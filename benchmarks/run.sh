#!/usr/bin/env bash
# Serve one benchmark app with trunk in release mode.
#   ./run.sh glory | leptos | dioxus
set -euo pipefail

app="${1:-}"
case "$app" in
  glory|leptos|dioxus) ;;
  *) echo "usage: $0 {glory|leptos|dioxus}" >&2; exit 1 ;;
esac

cd "$(dirname "$0")/$app"
exec trunk serve --release --open
