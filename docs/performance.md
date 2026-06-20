# Performance Notes

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
