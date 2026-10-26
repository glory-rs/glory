# Glory / Sapid WASM size probes

See [minimal-app attribution](../../docs/wasm-size-attribution.md) for the final
function-body breakdown and regex audit. `attribute.cjs` uses a name-preserving
Binaryen output and rejects any mismatch against the measured non-custom sections.

This Windows baseline workspace builds small applications against both sibling source
trees. `cases.json` defines the empty module, empty mounted app, text, interactive
Glory/Sapid buttons, each enabled-but-unused optional feature, all unused features,
and one executed representative for each optional feature group. This is an
application-size measurement, not the size of a complete widget family.

Every application uses the same exported `mount`/`unmount` entry points. The
button cases include a reactive counter. Used CSS parses and lowers a sheet;
Fluent resolves an Owner-owned message; resources run a named async loader;
hydration adopts real native-generated snapshot nodes; stream handoff restores
an inert resource block. Charts, editor, dates, code, JSON, files and chat mount
real widgets. The browser probe checks output, button updates, hydration identity,
resource completion, editor input and cleanup rather than treating a compile as
proof that the code was exercised.

`glory-ssr-used-in-browser` and `glory-stream-script-embedded` deliberately show
the cost of doing server work / embedding the external bootstrap in a browser
WASM. They are diagnostic cases, not recommended application architecture.
Sapid structural CSS, fonts and images are separate assets and are not embedded
by these probes. The regular Glory base CSS and widget-generated typed styles
are included wherever the runtime uses them.

## Reproduce

Requirements: Rust 1.98.0 with wasm32-unknown-unknown, wasm-bindgen **0.2.129**,
Node **24.3.0**, Binaryen npm **132.0.0**, and Playwright with installed Chrome.
The measured versions are recorded in every report; comparisons reject a
different recorded toolchain. Set `WASM_BINDGEN` (or `--bindgen`) to a matching
CLI if the command on PATH has another version.

```powershell
$env:TEMP = 'D:/BuildCache/sapid/tmp'
$env:TMP = $env:TEMP
$env:npm_config_cache = 'D:/BuildCache/sapid/wasm-size/npm-cache'
npm install --prefix D:/BuildCache/sapid/wasm-size/tools --save-exact binaryen@132.0.0
$env:WASM_BINDGEN = 'D:/BuildCache/glory-website/tools/wasm-bindgen-0.2.129-x86_64-pc-windows-msvc/wasm-bindgen.exe'
node benchmarks/wasm-size/run.cjs --label baseline
node benchmarks/wasm-size/browser.cjs D:/BuildCache/sapid/wasm-size/runs/baseline
# Checked-in optimized baseline (or pass a previous run report):
node benchmarks/wasm-size/run.cjs --label candidate --compare benchmarks/wasm-size/baseline.json --check
node benchmarks/wasm-size/browser.cjs D:/BuildCache/sapid/wasm-size/runs/candidate
```

`--only glory-text,sapid-button` selects a quick subset. Browser checks resume
only cases with an identical WASM hash and the same source digest/browser version.
Set `PLAYWRIGHT_MODULE` and `BROWSER_EXECUTABLE` when using other installed paths.
The default Playwright module comes from `tests/playwright/node_modules/playwright`.
`--source-from D:/BuildCache/sapid/wasm-size/runs/baseline/source --label replay`
rebuilds an archived source snapshot at the same compilation path after checking
its file hashes. Restored input timestamps are refreshed so Cargo cannot reuse
artifacts for newer sources over an old Windows copy timestamp. It does not
change either original repository. The runner lock
prevents concurrent matrices from replacing each other's staged sources; after
a forced process termination, inspect the recorded PID before removing a stale
`runner.lock`.

All compiler outputs, temporary files, tools and compression results live under
`D:/BuildCache/sapid/wasm-size`; Cargo's existing D-drive home is reused. The
runner verifies the resolved `cargo metadata` output directory before building.
It snapshots the current (possibly dirty) Glory/Sapid sources, records file hashes,
and archives them per run. Builds always use the same frozen source path, which
avoids changing embedded Rust paths between baseline and candidate. Do not run
two matrices concurrently against the same build root.

## Recorded artifacts

Each run contains `sources.json`, an archived `source/`, its exact dependency
`Cargo.lock`, `report.json`, `summary.md` and browser evidence. Per case it keeps:

- Rust compiler artifact and compiler JSON output (including the final artifact's
  `fresh` state), resolved feature tree, JS bindings and a test page.
- wasm-bindgen output and Binaryen `-Oz` output separately. Binaryen also removes
  the debug names section by default; the whole stage delta is **not** all
  executable-code reduction.
- Raw bytes, SHA-256, Brotli quality 11 and gzip level 9 for final WASM and JS.
  JS totals include the entry module and transitively imported wasm-bindgen
  snippets, compressed separately as network resources; the test loader is excluded.
  Brotli is a transport-size estimate, not decompressed memory or RSS.
- WASM section payload sizes. `analyze.cjs path/to/bindgen.wasm` lists the largest
  actual function bodies while debug names are still available; its byte counts
  are uncompressed function bodies, not exclusive retained-size attribution.

Release settings are `opt-level=z`, full LTO, one codegen unit, no debug info,
and strip=debuginfo. Ambient Rust flags/profile overrides are removed. Cold/warm
build times are not controlled: `buildMs` is diagnostic and must not be presented
as a compile-speed benchmark. Size-only probes do not establish runtime speed.

The `reference` column points to the comparison case for each marginal cost.
Enabled-but-unused features may produce small code-layout/compression differences;
their dependency count does not imply that all dependency code reached the WASM.
Used feature examples do not cover all possible options, languages, chart types
or editor configurations. Keep the full widget/runtime suites as separate gates.

`--check` rejects growth above 0.5% for final WASM, Brotli WASM or JS, with floors
of 1 KiB for WASM and 512 bytes for Brotli/JS. A reviewed baseline update is
required for intended growth. Unused-feature deltas should also be inspected;
the aggregate gate is not a proof that a newly enabled subsystem was eliminated.
When both cases of a reference pair are present, unused-feature premiums also
have independent growth limits of 1 KiB WASM / 512 B Brotli. This prevents a
shared runtime reduction from hiding new retention in an unused feature.

`node benchmarks/wasm-size/test.cjs` verifies the gate with saved real measurements,
including a regression hidden by a shared runtime reduction.
`node benchmarks/wasm-size/compare.cjs candidate/report.json benchmarks/wasm-size/baseline.json --check`
checks saved reports without rebuilding. Use a new run label for every build.

After reviewing a complete run, `node benchmarks/wasm-size/capture.cjs
D:/BuildCache/sapid/wasm-size/runs/candidate benchmarks/wasm-size/baseline.json`
explicitly records the new baseline. Capture rejects incomplete matrices or
missing browser evidence. Do not update the baseline automatically after a
failed size check; review the intended behavior and resulting payload first.
