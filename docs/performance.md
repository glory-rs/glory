# Performance Notes

## DOM retirement follow-up — 2026-10-23

The parent lookup added for DOM keyboard/drag routing used `HashMap::retain`
for every disposed subtree. Clearing a keyed list disposes many row subtrees,
so each row scanned unrelated live parent entries and the table's retained
capacity. Repeated row teardown could approach quadratic work. A 10k run also
left a larger table allocation for later 1k clears; the main benchmark's setup
includes those expensive 10k clears outside the timed action interval.

`Dom::dispose` now removes parent and pending-size entries by each `NodeId`
in the mapper's complete semantic subtree. It includes parked descendants;
no source scan, new cache, ID lifetime change or capacity shrinking is involved.
The normal input/layout-queue filters and observer retirement remain in place.
The row workload does not subscribe to Resize, so the large regression here
comes from the parent lookup rather than a populated measurement cache.

The before/release snapshots differ only in `crates/web/src/browser/dom.rs`.
Both were freshly built in independent D: release targets, opt-level=3/LTO/one
codegen unit, wasm-bindgen/CLI 0.2.129 and no wasm-opt. The benchmark app, seed,
CSS, data generator and timing harness are byte-identical. Chrome 155.0.8059.84,
1280×900, scale 1; before→release then release→before, fresh contexts, three
warmups and 12 measured samples per operation/round: **432 samples**, 24 per
variant/operation. These are contemporaneous paired results, not reused values
from the earlier three-framework run.

Median ms. Wall is click-to-two-RAF; script is the CDP counter delta including
common bridge/harness work. This is a local full-DOM benchmark, not FPS,
presentation latency, RSS or an official HTML-table score.

| Operation | Wall before → after | Script before → after | Script reduction |
|---|---:|---:|---:|
| create_1k | 109.75 → 103.40 | 78.69 → 73.10 | 7.1% |
| replace_1k | 221.50 → 143.80 | 195.08 → 110.52 | 43.3% |
| update_10th | 11.20 → 11.10 | 0.68 → 0.70 | -3.0% |
| select_row | 8.05 → 8.10 | 1.10 → 1.12 | -2.0% |
| swap_rows | 10.85 → 11.10 | 0.59 → 0.64 | -8.1% |
| remove_row | 11.10 → 10.25 | 0.95 → 0.67 | 28.8% |
| append_1k | 113.50 → 110.15 | 73.07 → 77.07 | -5.5% |
| create_10k | 1131.60 → 1089.40 | 788.80 → 814.76 | -3.3% |
| clear_1k | 150.15 → 40.30 | 148.41 → 36.23 | 75.6% |

The improvement is concentrated in replacement and teardown. Creation,
append and tiny updates are essentially unchanged; do not treat their small
percentage differences as gains from this patch.

Separate diagnostic runs wrap DOM methods and use a sampling profiler. They
are excluded from the 432 timing samples and are not additional median scores:

| Diagnostic | Busy sampled ms before → after | DOM sink inclusive ms before → after |
|---|---:|---:|
| replace_1k | 206.09 → 141.09 | 141.75 → 77.74 |
| clear_1k | 103.15 → 26.16 | 80.23 → 8.01 |
| clear_10k | 10227.81 → 386.48 | 9528.39 → 119.39 |
| clear_1k_after_10k | 211.88 → 34.88 | 179.26 → 12.26 |

The 10k clear previously spent about 9.53 s inside the DOM sink; after the
change it spends about 119 ms there. Owner disposal is still present (about
177 ms inclusively in that optimized diagnostic) and is now a larger part of
the remaining cost. Inclusive families overlap and must not be added together.
The profiler resolves the adapter sink rather than a precise inlined retain
instruction; the paired source change and unchanged native-operation counts
are the evidence for the parent-table attribution.

All six diagnostic cases have identical native call/attribute counts and live
DOM/graph census across variants. Replacement creates 8,000 elements, writes
32,000 attributes and removes 1,000 row roots in both; clear-1k removes 1,000
roots, clear-10k removes 10,000. Speedup comes from less metadata work.

Correctness checks passed for both builds: seeded row data for all operations
and timing samples, surviving Element/Text Node/semantic IDs through update,
select, swap, remove and append, old descendants disconnected on replacement,
all 10,000 row rectangles equal, six viewport PNG checkpoints byte-identical
(including the middle of 10k rows), and cleanup back to zero rows/labels,
10 semantic nodes, 3 reactive nodes and 13 Owners.

Full regression on the frozen validation tree:

| Gate | Result |
|---|---|
| Windows workspace / fork tests | 428 / 289 passed |
| Linux workspace / fork tests | 428 / 289 passed |
| Browser | 24/24 suites passed |
| Windows real windows / UIA | shell + AccessKit windows and actual two-window UIA client passed |
| Linux real windows / paint | X11/Wayland × shell/AccessKit, CPU raster passed |
| Formatting / Clippy | validation workspace, native and WASM all targets, fixture passed |
| SSR / streaming | 4 fixture tests, actual progressive delivery, cancellation and slow-consumer backpressure passed |

The browser regression also reruns the real fractional-height lists with 64
and 1024 entries: both maintain zero anchor drift through 70 fractional changes,
live descendant/Text identities, equal settled PNGs and identical final logical
and physical Home positions. The original 0.1/0.15 px checks are unchanged.

The validation tree started as a copy of the measured release source. Seven
files were normalized with rustfmt. Full-feature Clippy then required equivalent
let-chains in existing native/Web drag and theme branches; no-shell Clippy
required gating private desktop drag state/initializers by `shell`. The fixes
are also in the working tree. `native-lint-fix.py`, `web-lint-amendments.json`,
`dom-lint.diff` and `drag-lint.diff` preserve the exact amendments. The initial
and second failed attempts remain in `windows-initial.txt`, `linux-initial.txt`,
`windows-second.txt` and `linux-second.txt`; only subsequent complete runs count
as accepted. Later parallel worktree edits are excluded.

After those gate fixes, `final/source` was built independently again. Its
benchmark/library input hashes exactly match the accepted validation tree.
The same workload passed another **108 samples**, six equal PNG checkpoints
and a full descendant/Text/semantic identity and 10k-geometry differential
against the measured release build. This is a final-source verification run,
separate from the 432-sample causal experiment above:

| Final-source verification | Wall median | Script median |
|---|---:|---:|
| replace_1k | 119.75 ms | 91.76 ms |
| clear_1k | 40.40 ms | 36.95 ms |
| create_10k | 1053.40 ms | 767.60 ms |


Everything is under `D:/BuildCache/glory-dom-retirement-20261023/`:
`before/source/inputs.json`, `release/source/inputs.json`, `validation/inputs.json`,
`paired/evidence.json`, `integrity/evidence.json`, `browser/evidence.json`,
`final/source/inputs.json`, `final-benchmark/evidence.json`, `summary.json`,
raw CPU profiles and final platform logs.
The [evidence index](/D:/BuildCache/glory-dom-retirement-20261023/evidence-index.json)
verifies sources, independent output directories, data/geometry/picture hashes,
method counts, accepted regressions and retained failed attempts.

## Current three-framework rerun — 2026-10-23

The current Glory snapshot was rebuilt alongside **Leptos 0.8.19** and
**Dioxus 0.7.9**, using fresh independent release targets on D:, opt-level=3,
LTO, one codegen unit and wasm-bindgen/CLI 0.2.129; no wasm-opt. All three
strict WASM Clippy and app formatting checks passed. The 15 shared/app build
inputs, including harness, data, CSS and lockfiles, are byte-identical to the
earlier semantic-rows-v1 comparison. Only Glory library sources differ.

Chrome **155.0.8059.84**, Windows/MSVC Rust 1.98.0, Intel Core i7-14700KF,
1280×900, scale 1. Two sequential rounds used fresh contexts in opposite order:
Leptos→Glory→Dioxus, then Dioxus→Glory→Leptos. Each operation warmed three
times and measured 12 samples per round: **24 samples per cell, 648 total**.
The table pools both rounds and reports medians. The browser had its own
process/profile; this is a local machine run, not an isolated lab measurement.
Sources were frozen before building. Later concurrent working-tree changes
did not enter these binaries; their paths are recorded in `validation.json`.

All three passed the independent seeded-data oracle, nine operations,
keyed swap/replacement checks, all 10,000 real row boxes and cleanup checks.
Every measured sample's final row hash/count agrees across frameworks and
rounds. All six create-1k checkpoint viewport PNGs are byte-identical.

Median **click to two foreground RAF callbacks**, ms, lower is better.
Setup/data validation is outside timing; this does not establish actual
display completion, FPS or an official HTML-table score.

| Operation | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| create_1k | 90.30 | 40.00 | 32.65 |
| replace_1k | 306.55 | 47.70 | 33.35 |
| update_10th | 14.35 | 15.40 | 11.25 |
| select_row | 9.10 | 267.70 | 8.10 |
| swap_rows | 10.60 | 10.65 | 9.70 |
| remove_row | 12.50 | 13.90 | 10.55 |
| append_1k | 177.95 | 89.65 | 37.95 |
| create_10k | 1625.25 | 739.20 | 341.85 |
| clear_1k | 152.65 | 21.60 | 9.10 |

Median CDP **ScriptDuration counter delta**, ms. This includes common
harness/binding-bridge work and is diagnostic; inclusive CPU metrics must not
be summed.

| Operation | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| create_1k | 63.93 | 14.87 | 4.75 |
| replace_1k | 266.76 | 23.06 | 8.09 |
| update_10th | 0.81 | 0.50 | 1.14 |
| select_row | 1.12 | 262.02 | 0.93 |
| swap_rows | 0.85 | 0.22 | 1.08 |
| remove_row | 1.18 | 0.41 | 1.07 |
| append_1k | 125.42 | 33.39 | 6.19 |
| create_10k | 1224.57 | 293.16 | 54.99 |
| clear_1k | 151.33 | 19.40 | 2.94 |

Per-round create-10k medians expose the repeatability limits:

| App | Round 1 wall / script ms | Round 2 wall / script ms |
|---|---:|---:|
| glory | 2067.90 / 1465.37 | 1519.95 / 1084.01 |
| leptos | 871.10 / 364.11 | 533.20 / 207.07 |
| dioxus | 353.60 / 57.01 | 321.55 / 52.91 |

Glory still trails on bulk creation/replacement/append/clear. Small updates,
reorders and removal are much closer in absolute cost; RAF waiting limits the
wall-time resolution for these short operations. There is no overall ranking.

Glory replacement varied substantially: 373.60 ms versus 209.50 ms
across the two rounds. Treat the pooled median as a local observation; do not
interpret small differences or historical deltas as a precise optimization effect.

Source inspection found a newly added cost in the current DOM adapter:
`Dom::dispose` calls `parents.retain(...)` for every disposed subtree, scanning
the whole live parent table. Disposing many row subtrees can therefore repeat
a full-table scan. This is a concrete profiling candidate for clear/replacement
and untimed setup cleanup; source inspection alone does not quantify its share
of the observed time. The frozen-source diff is retained as `parent-map-diff.txt`.
This rerun keeps that code intact; it does not substitute an older Glory binary.

The mixed-workload Leptos selection outlier is reported as observed. Separate
fresh-context diagnostics are excluded from the main 648 samples:

| App | Selection script, first 1k list | Selection script, after 30 replacements |
|---|---:|---:|
| glory | 1.19 ms | 1.54 ms |
| leptos | 1.27 ms | 6.78 ms |
| dioxus | 0.76 ms | 1.34 ms |

This establishes history sensitivity in the workload, not its root cause or
a general selection-performance ranking.

Ten-thousand-row census, outside timing:

| App | Elements | Text nodes | Comments | Attributes |
|---|---:|---:|---:|---:|
| glory | 80,008 | 30,006 | 4 | 320,037 |
| leptos | 80,008 | 30,006 | 1 | 120,009 |
| dioxus | 80,008 | 30,006 | 0 | 140,015 |

Glory's live 10k state: 80,010 semantic nodes, 60,003 reactive
nodes and 90,013 Owners. After clear it returns to the baseline
(10 semantic nodes, 3 reactive nodes, 13 Owners, zero rows/labels). Counts do
not measure RSS; no memory-byte claim is made.

Historical deltas describe different sessions, not a simultaneous causal A/B:

| Glory operation | Earlier wall → current ms | Earlier script → current ms |
|---|---:|---:|
| create_1k | 133.65 → 90.30 | 103.50 → 63.93 |
| replace_1k | 173.80 → 306.55 | 143.06 → 266.76 |
| append_1k | 169.55 → 177.95 | 129.07 → 125.42 |
| create_10k | 2809.90 → 1625.25 | 2156.60 → 1224.57 |
| clear_1k | 83.05 → 152.65 | 79.79 → 151.33 |

Use the earlier [paired bulk experiment](bulk-dom-optimization-2026-10-23.md)
for the controlled optimization comparison. The subsequent anchor correction
does not make this full-`each` workload a variable-height/virtual-list benchmark.

Frozen input manifest: `D:/BuildCache/glory-framework-rerun-20261023/source/inputs.json`.
Raw results: `round-1/evidence.json`, `round-2/evidence.json` under that build root.
The [evidence index](/D:/BuildCache/glory-framework-rerun-20261023/evidence-index.json)
contains all medians, per-round values, source/build/site/driver/PNG hashes,
independent target metadata and the separate selection diagnostic. No production
framework code was changed for this rerun. Runner changes add explicit ordering,
per-run screenshots/source preservation and an untimed DOM census.

## Fractional list anchor correction — 2026-10-23

The follow-up fixes the adaptive list's host scroll quantization residual.
Independently built before/final fixtures reproduce **0.375 → 0 px** drift with
both 64 and 1024 measurement entries, using actual fractional DOM row heights
and the unchanged 0.1 px anchor tolerance. Each configuration passed 70 repeated
fractional changes, live descendant/Text Node identity checks, 80 far seeks and
73 viewport PNG checkpoints. Corresponding settled images agree byte for byte;
both final logical and physical offsets are zero. All 24 browser suites pass.
This is correctness evidence, not a new bulk timing or presentation-latency run.
See the [fix, complete regression scope and frozen-source evidence](anchor-residual-fix-2026-10-23.md).

## Bulk DOM optimization — 2026-10-23

The paired follow-up kept the benchmark app/data/CSS/harness byte-identical and
used independently built before/after WASM, two reversed-order rounds and 432
samples. Median script time: create-10k **1127.54 → 730.45 ms (-35.2%)**,
replace-1k **129.69 → 88.48 ms (-31.8%)**, clear-1k **40.56 → 31.06 ms (-23.4%)**.
Ten-thousand-row Owners fell from 130,016 to 90,013, attributes from 560,061 to
320,037 and comments from 30,010 to 4; semantic nodes and bindings stayed live.
Six viewport PNG checkpoints and all 10,000 row rectangles agree, with surviving
row/descendant/text/semantic identities preserved across updates and moves.

See the [detailed implementation, results and evidence](bulk-dom-optimization-2026-10-23.md).
The full native workspace passed 419 tests and the actual Windows/UIA probes.
Browser suites: 23/24 passed; the adaptive suite's 0.375 px anchor discrepancy
also reproduces identically on the unoptimized snapshot. Do not describe that
historical run as green or combine this paired baseline with the historical timings below.
The subsequent anchor correction and its separate complete regression are linked above.

## Semantic browser comparison before bulk optimization — 2026-10-23

The three local apps have been migrated to **semantic-rows-v1**. Glory uses
`web`, `launch_with`, public handwritten semantic builders and the complete keyed
`each`; there is no `BrowserHolder`, old `Each`, template cloning, manual row DOM
patching, virtual scrolling or compatibility feature alias. Dioxus is pinned to
0.7.9 and Leptos to 0.8.19. All three use the same Rust input generator, seed,
visible element/text tree, CSS and nine operations. Framework attributes and
comment markers remain part of each implementation's measured cost.

This is a **local full-DOM workload, not an official js-framework-benchmark
table score**: the finite semantic API currently maps its table/row/cell roles to
divs, and all three adapters now use that same structure. The old official runner
fails explicitly instead of silently submitting a different DOM contract.

All three passed the independent seeded-data oracle, nine operation checks,
keyed-node preservation on swap, replacement/removal checks, 10,000 actual row
boxes and repeated clear/recreate checks. The viewport screenshots at the same
create-1k checkpoint are byte-identical. Every measured sample's final row-data
SHA-256 and count agree across frameworks. Glory returns to rows=0, labels=0,
10 semantic nodes, 3 live reactive nodes and 16 Owners after clear.

One round used Chrome **155.0.8059.84**, 1280×900 at scale 1, fresh contexts and
order Leptos→Glory→Dioxus. Each operation had 3 untimed warmups and 12 samples
(324 measured samples total). The browser process/profile was independent of
other tasks; temporary profiles and all build/test output were on D:.
Windows Rust 1.98.0/MSVC, three independent Cargo targets, opt-level=3, LTO and
one codegen unit; all use wasm-bindgen/CLI 0.2.129, with **no wasm-opt**. The
accepted frozen library/app sources are in `source-keyed/inputs.json`; later
working-tree edits cannot enter these binaries. All three WASM/all-targets
strict Clippy checks passed, as did formatting and runner syntax checks.
After the Dioxus key correction, Glory and Dioxus rebuilt. Leptos reused only
its own isolated target with byte-identical app/manifest/lock/shared-data inputs;
the compiler-artifact freshness flags and exact WASM hashes are preserved.

Median **click to two foreground RAF callbacks**, milliseconds, with setup and
correctness snapshots excluded. This includes waiting for frame callbacks; it
does not establish presentation latency or FPS:

| Operation | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| create_1k | 133.65 | 43.35 | 49.80 |
| replace_1k | 173.80 | 49.75 | 47.30 |
| update_10th | 14.40 | 14.60 | 22.60 |
| select_row | 11.00 | 250.60 | 9.45 |
| swap_rows | 12.30 | 10.80 | 11.15 |
| remove_row | 14.65 | 12.80 | 15.95 |
| append_1k | 169.55 | 66.65 | 67.70 |
| create_10k | 2809.90 | 509.30 | 554.90 |
| clear_1k | 83.05 | 20.70 | 10.50 |

Median CDP **ScriptDuration counter delta**, milliseconds. The common harness
and binding bridge have some cost; these are diagnostics, not the official
tracing runner's isolated script metric:

| Operation | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| create_1k | 103.50 | 16.17 | 7.25 |
| replace_1k | 143.06 | 24.29 | 10.53 |
| update_10th | 0.72 | 0.65 | 1.67 |
| select_row | 1.59 | 248.11 | 1.14 |
| swap_rows | 0.68 | 0.26 | 1.50 |
| remove_row | 0.81 | 0.42 | 1.34 |
| append_1k | 129.07 | 25.28 | 11.98 |
| create_10k | 2156.60 | 194.23 | 95.41 |
| clear_1k | 79.79 | 18.15 | 4.87 |

The main gap is bulk work. Glory create-10k is 2809.90 ms versus Leptos 509.30
and Dioxus 554.90 ms; its script counter is 2156.60 ms versus 194.23/95.41 ms.
Create/replace/append/clear also trail. Fine updates, swap and removal are much
closer in absolute time; frame-waiting limits the resolution of their wall time.
No overall score or statistical-significance claim is made from this one ordered
round on a machine running other work.

**Selection is history-sensitive.** The mixed sequence's Leptos select result
(250.60 ms wall / 248.11 ms script) is anomalous, not a general Leptos selection
baseline. A separate fresh-page probe with alternating selection on a fixed
1k list found the following script-counter medians; it is not pooled into the
main table and uses its own counter boundary:

| App | After 1 create | After 30 replacements |
+|---|---:|---:|
| glory | 2.259 | 2.165 |
| leptos | 2.204 | 11.110 |
| dioxus | 1.120 | 1.260 |

This confirms history sensitivity in this test but does not identify a leak or
its root cause. Do not use the mixed selection outlier to rank the frameworks.

A separate 10k census (outside timing) shows equal visible element/text counts,
but meaningful implementation overhead:

| App | Elements | Text nodes | Comments | Attributes |
+|---|---:|---:|---:|---:|
| glory | 80008 | 30006 | 30010 | 560061 |
| leptos | 80008 | 30006 | 1 | 120009 |
| dioxus | 80008 | 30006 | 0 | 140015 |

Glory's 10k state additionally reports 80,010 semantic nodes, 60,003 live graph
nodes and 130,016 Owners. These are live-count observations, not RSS or proof
of the timing root cause. They motivate profiling per-node construction,
Owner/Binding allocation, mapper work and DOM metadata/markers before changing
the contract. The earlier million/100-million-row virtual-list work mounted
only a bounded window and cannot establish full-`each` throughput.

Generated WASM bytes after bindgen (not compressed, and not whole download size):
Glory 817,189; Leptos 278,497; Dioxus 457,457. No cross-framework final bundle
size ranking is inferred without accounting for JS and Dioxus's snippets.

Initial conformance rejected Dioxus's expression-wrapped row loop because swap
replaced DOM nodes. The accepted version puts the keyed div directly in the
RSX loop. Earlier packaging assumed an underscore binary filename; the build
script now takes the WASM path from Cargo's compiler-artifact record. These
diagnostics and the failed pre-correction run are retained, never mixed into
the accepted timings. The shared MCP browser was occupied, so this run used a
separate Playwright-launched Chrome with D:-backed temporary profiles.

Raw records and the preserved measured driver sources:
`D:/BuildCache/glory-framework-compare/browser-keyed-round/evidence.json`.
Build/source/probe/cleanup hash index:
`D:/BuildCache/glory-framework-compare/comparison-evidence-2026-10-23.json`.
The new runner is `benchmarks/compare.ps1`; see the benchmark README for setup.
No production framework source was changed by this benchmark migration.

## Historical measurements (previous architecture)

This file keeps reproducible local measurements that can be compared before
deeper optimization work. The first baseline is intentionally raw cargo wasm:
it does not include `wasm-bindgen`, minification, compression, CDN headers, or
Binaryen passes unless called out in the `Kind` column.

## Wasm Size Baseline

Date: 2026-06-11

Command:

```powershell
./benchmarks/wasm-size.ps1
```

Local tool availability during this run:

- `wasm-opt`: not found on `PATH`, so `wasm-opt -Oz` rows were not produced.
- `wasm-bindgen`: not found on `PATH`, so final bindgen package size was not
  measured.

| Example | Variant | Profile | Kind | Bytes | KiB | MiB |
| --- | --- | --- | --- | ---: | ---: | ---: |
| _test-size | minimal | debug | raw cargo wasm | 42772055 | 41769.6 | 40.79 |
| _test-size | minimal | release | raw cargo wasm | 1299394 | 1268.9 | 1.24 |
| counter | web-csr | debug | raw cargo wasm | 42048282 | 41062.8 | 40.1 |
| counter | web-csr | release | raw cargo wasm | 1350085 | 1318.4 | 1.29 |
| router-basic | web-csr+routing | debug | raw cargo wasm | 45077943 | 44021.4 | 42.99 |
| router-basic | web-csr+routing | release | raw cargo wasm | 1563804 | 1527.2 | 1.49 |
| todomvc-fullstack | web-csr+server-fn | debug | raw cargo wasm | 46334731 | 45248.8 | 44.19 |
| todomvc-fullstack | web-csr+server-fn | release | raw cargo wasm | 1693113 | 1653.4 | 1.61 |

Interpretation:

- The minimal `_test-size` fixture is 1.24 MiB raw release wasm, about
  49.5 KiB smaller than the counter release build.
- Routing adds about 213.7 KiB to the raw release wasm over the counter
  baseline in this sample.
- The fullstack/server-fn sample adds about 335.0 KiB over the counter
  baseline and about 126.2 KiB over the routing sample.
- These numbers are regression guardrails, not final download sizes. Final
  payload tracking still needs `wasm-bindgen`, `wasm-opt -Oz`, and compressed
  asset measurements in CI.
- `./benchmarks/wasm-size.ps1 -Json` now emits tool availability and skipped
  case metadata alongside rows, so missing `wasm-bindgen` and `wasm-opt` are
  machine-readable in reports.

## Official JS Framework Benchmark

The local `benchmarks/` apps are useful for fast iteration, but official
numbers should come from krausest's Chrome tracing runner. Use:

```powershell
./benchmarks/official-js-framework-benchmark.ps1
```

The script clones or reuses `JS_FRAMEWORK_BENCHMARK_REPO`, generates
`frameworks/keyed/glory-rs` and `frameworks/keyed/dioxus-rs` adapters from the
local benchmark apps, builds them with `trunk build --release`, runs
`npm run bench -- --framework keyed/glory-rs keyed/dioxus-rs`, then writes
status and copied result artifacts under
`target/benchmark-report/official-js-framework/`. It also writes
`official-js-framework-summary.md` and `.json` with median/range tables for the
official `total`, `script`, and `paint` metrics.

Use `-SkipBench` for adapter/build validation only, or `-ChromeBinary` when
Chrome/Edge is not discoverable. On this Windows machine Chrome exists at
`C:\Program Files\Google\Chrome\Application\chrome.exe`.

Use `-BaselineName name` to preserve a named baseline under
`target/benchmark-report/official-js-framework/baselines/name/`; pass
`-CompareBaseline name` on a later run to add baseline deltas to the summary.
For optimization A/B work that should isolate Glory from other framework
builds, pass `-GloryOnly`. Baselines are not replaced unless
`-OverwriteBaseline` is set.

Stable local A/B example:

```powershell
./benchmarks/official-js-framework-benchmark.ps1 `
  -GloryOnly `
  -Benchmarks 01_,02_,03_,04_,05_,06_,07_,08_,09_ `
  -Count 5 `
  -Headless `
  -NoThrottling `
  -BaselineName before-e9

./benchmarks/official-js-framework-benchmark.ps1 `
  -GloryOnly `
  -Benchmarks 01_,02_,03_,04_,05_,06_,07_,08_,09_ `
  -Count 5 `
  -Headless `
  -NoThrottling `
  -CompareBaseline before-e9 `
  -BaselineName after-e9
```

Local official CPU smoke, 2026-06-11:

```powershell
./benchmarks/official-js-framework-benchmark.ps1 `
  -SkipInstall `
  -Benchmarks 01_,02_,03_,04_,05_,06_,07_,08_,09_ `
  -Count 1 `
  -Headless `
  -NoThrottling
```

This is the official Chrome/Puppeteer tracing path with one measured sample per
CPU benchmark. It is a CI-style smoke baseline, not a publishable median run.
Result JSON and the built result viewer are under
`target/benchmark-report/official-js-framework/`.

| Benchmark | Framework | Total ms | Script ms | Paint ms |
| --- | --- | ---: | ---: | ---: |
| `01_run1k` | dioxus-rs v0.7 keyed | 28.1 | 6.1 | 21.4 |
| `01_run1k` | glory-rs local keyed | 43.8 | 22.0 | 21.4 |
| `02_replace1k` | dioxus-rs v0.7 keyed | 32.2 | 10.0 | 21.7 |
| `02_replace1k` | glory-rs local keyed | 62.6 | 41.8 | 20.5 |
| `03_update10th1k_x16` | dioxus-rs v0.7 keyed | 10.1 | 1.2 | 8.5 |
| `03_update10th1k_x16` | glory-rs local keyed | 9.1 | 0.7 | 8.1 |
| `04_select1k` | dioxus-rs v0.7 keyed | 2.4 | 1.2 | 0.8 |
| `04_select1k` | glory-rs local keyed | 3.4 | 2.4 | 0.7 |
| `05_swap1k` | dioxus-rs v0.7 keyed | 3.7 | 1.1 | 2.4 |
| `05_swap1k` | glory-rs local keyed | 3.1 | 0.5 | 2.3 |
| `06_remove-one-1k` | dioxus-rs v0.7 keyed | 31.4 | 11.9 | 18.8 |
| `06_remove-one-1k` | glory-rs local keyed | 6.2 | 0.5 | 4.0 |
| `07_create10k` | dioxus-rs v0.7 keyed | 307.5 | 66.3 | 238.7 |
| `07_create10k` | glory-rs local keyed | 643.0 | 388.5 | 251.7 |
| `08_create1k-after1k_x2` | dioxus-rs v0.7 keyed | 39.0 | 8.0 | 30.5 |
| `08_create1k-after1k_x2` | glory-rs local keyed | 61.0 | 30.2 | 30.3 |
| `09_clear1k_x8` | dioxus-rs v0.7 keyed | 7.0 | 3.3 | 0.5 |
| `09_clear1k_x8` | glory-rs local keyed | 16.2 | 15.4 | 0.5 |

The first full official run caught two conformance issues before producing the
green baseline: composite `Each` rows were not reattaching their descendant DOM
nodes during CSR moves, and the benchmark remove icon was not reliably
clickable in the official headless runner. Both are fixed in this checkout.

### Focused Create10k Rerun

E12 validation reran only `07_create10k` with five samples after optimizing the
Glory benchmark data generator and CSR delegated click registration:

```powershell
./benchmarks/official-js-framework-benchmark.ps1 `
  -SkipInstall `
  -Benchmarks 07_create10k `
  -Count 5 `
  -Headless `
  -NoThrottling
```

| Benchmark | Framework | Samples | Total median | Total range | Script median | Script range | Paint median | Paint range |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `07_create10k` | dioxus-rs v0.7 keyed | 5 | 328.9 | 26.5 | 71.7 | 7.8 | 253.9 | 18.1 |
| `07_create10k` | glory-rs local keyed | 5 | 319.8 | 126.4 | 80.3 | 25.5 | 241.2 | 100.8 |

Glory now matches the official create10k total time and is faster on paint in
this focused run. The remaining script-only gap is about 8.6 ms and is tied to
the per-row reactive scope/static-subtree overhead tracked under E9.

E9 added automatic CSR compact-fill for ordinary builder element wrappers:
fresh CSR mounts now keep non-hydrated, self-static elements as native DOM
instead of allocating a View for each wrapper, while reactive/listener nodes
remain normal Views and dynamic descendants keep their fixed native parent.
The same focused command was rerun after that change:

| Benchmark | Framework | Samples | Total median | Total range | Script median | Script range | Paint median | Paint range |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `07_create10k` | dioxus-rs v0.7 keyed | 5 | 326.0 | 26.3 | 69.3 | 5.4 | 252.3 | 20.4 |
| `07_create10k` | glory-rs local keyed | 5 | 330.4 | 34.1 | 73.8 | 13.7 | 244.0 | 21.2 |

The script gap narrowed to about 4.5 ms in this run; total time remains close
and is sensitive to paint variance.

## Local Benchmark Report

Use this wrapper to generate a repeatable local bundle:

```powershell
./benchmarks/report.ps1
```

It runs:

- `cargo bench -p glory-core --bench command_wire`
- `cargo bench -p glory-core --bench event_handlers`
- `cargo bench -p glory-core --features web-ssr --bench each_reorder`
- `cargo bench -p glory-core --features web-ssr --bench scheduler`
- `cargo bench -p glory-core --features web-ssr --bench ssr_stream`
- `./benchmarks/wasm-size.ps1 -Json`
- Playwright CSR/hydration/fullstack projects when `npm` and `tests/playwright`
  are available. Missing app URLs are recorded in `playwright-status.json`
  instead of silently disappearing from the report.

The report summary, logs, wasm size JSON, and Playwright status JSON are written
to `target/benchmark-report/`; Criterion HTML remains under
`target/criterion/`.

The event-handler benchmark isolates 1k/10k same-DOM construction with and
without click handlers, plus command-stream dispatch lookup against empty and
populated handler registries. The scheduler benchmark covers a shared `Cage`
patching 100 and 1000 subscribed views plus command queue batch drain with and
without coalescing. The SSR stream benchmark compares full `render_string()`
output with collecting `render_stream()` chunks for 100 and 1000 row pages.

## Event Handler Baseline

Date: 2026-06-19

Command:

```powershell
cargo bench -p glory-core --bench event_handlers -- --sample-size 10 --warm-up-time 0.1 --measurement-time 0.1
```

Representative local results:

| Benchmark | Mean Range |
| --- | ---: |
| `event_handler_install/build_1000_rows_no_handlers` | 182.37-184.81 us |
| `event_handler_install/build_1000_rows_click_handlers` | 454.10-486.80 us |
| `event_handler_install/build_10000_rows_no_handlers` | 3.0247-3.7970 ms |
| `event_handler_install/build_10000_rows_click_handlers` | 4.2722-4.5484 ms |
| `event_dispatch_lookup/missing_click_10000_rows` | 75.931-77.746 ns |
| `event_dispatch_lookup/registered_click_10000_handlers_last_row` | 106.10-108.67 ns |

Interpretation:

- Command-stream click handler registration adds about 1 ms to a 10k-row
  synthetic build on this machine.
- Dispatch lookup/restore remains sub-microsecond even with 10k registered
  row handlers, so `create10k` tuning should prioritize bulk DOM command count
  and browser-side apply cost before inventing a separate row-delegation API.

## Scheduler And SSR Baseline

Date: 2026-06-11

Commands:

```powershell
cargo bench -p glory-core --features web-ssr --bench scheduler -- --sample-size 10 --warm-up-time 0.1 --measurement-time 0.1
cargo bench -p glory-core --features web-ssr --bench ssr_stream -- --sample-size 10 --warm-up-time 0.1 --measurement-time 0.1
```

Representative local results:

| Benchmark | Mean Range |
| --- | ---: |
| `scheduler_shared_signal/revise_100_subscribers` | 117.96-121.50 us |
| `scheduler_shared_signal/revise_1000_subscribers` | 1.2290-1.2758 ms |
| `command_queue_flush/take_raw_100_rows` | 32.814-36.248 ns |
| `command_queue_flush/take_coalesced_100_rows_3_text_writes` | 19.142-19.933 us |
| `command_queue_flush/take_raw_1000_rows` | 61.024-73.065 ns |
| `command_queue_flush/take_coalesced_1000_rows_3_text_writes` | 188.36-194.68 us |
| `ssr_render/render_string_100_rows` | 231.42-245.75 us |
| `ssr_render/render_stream_collect_100_rows` | 203.20-207.17 us |
| `ssr_render/render_string_1000_rows` | 2.0836-2.1884 ms |
| `ssr_render/render_stream_collect_1000_rows` | 2.1148-2.2274 ms |

Interpretation:

- Shared-signal patching scales close to linearly in subscriber count in this
  baseline.
- Raw command queue drain is effectively free because it is a `Vec` take.
- Coalescing has measurable cost and should stay opt-in for IPC/remote sinks.
- SSR stream chunk collection is comparable to full string rendering, and the
  holder now emits DOM-boundary `App` chunks instead of one giant app subtree
  chunk; real network soak tests can still be added at the adapter layer.
