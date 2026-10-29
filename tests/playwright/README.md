# Glory Playwright E2E

This directory contains the browser test for the current semantic counter.
It runs against a locally started or CI-managed counter server. Legacy
server-function, hot-reload and LiveView projects have been removed.

Install once:

```powershell
cd tests/playwright
npm install --package-lock=false
npm exec -- playwright install chromium
```

Run the following commands from `tests/playwright`. List projects:

```powershell
npm run list
```

Run a project by pointing it at a running app:

```powershell
$env:GLORY_COUNTER_URL = "http://127.0.0.1:8080"
npm run test:counter
```

The `counter` project targets the migrated semantic counter. Start it with
`./examples/counter/build.ps1 -Platform web -Run` from the repository root (see
that example's README for the matching wasm-bindgen CLI), then set `GLORY_COUNTER_URL` to
`http://127.0.0.1:8790`. The dedicated counter CI workflow builds its current WASM
entry and runs this project. The broader semantic browser acceptance suite is
in `examples/semantic-web`, driven by `scripts/test-semantic-browser.cjs`.

Environment variables:

- `GLORY_COUNTER_URL`: semantic counter Web entry.

When an environment variable is missing, that project skips its tests. This
keeps the suite safe to install and list before CI provisions browser servers.
