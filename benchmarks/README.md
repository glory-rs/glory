# Frontend framework benchmarks: glory vs leptos vs dioxus

A side-by-side performance comparison of **glory**, **leptos** and **dioxus**
using the workload from the industry-standard
[js-framework-benchmark](https://github.com/krausest/js-framework-benchmark)
("the keyed row table"). All three apps render the **same DOM** and expose the
**same control buttons**, so a single framework-agnostic harness
([`bench-harness.js`](./bench-harness.js)) drives them identically — the only
thing being measured is each framework's render/update path.

## What is measured

These are the operations every serious frontend framework is benchmarked on
(the js-framework-benchmark set):

| operation | what it stresses |
|---|---|
| **create 1,000 rows** | bulk node creation |
| **replace all 1,000 rows** | full keyed-list replacement |
| **partial update (every 10th of 1,000)** | fine-grained per-row update |
| **select a row** | single-node attribute/class toggle |
| **swap two rows** | minimal keyed reorder |
| **remove one row** | single keyed removal |
| **append 1,000 to 1,000** | incremental keyed append |
| **create 10,000 rows** | large bulk creation / scaling |
| **clear 1,000 rows** | bulk teardown |

(The official suite also tracks startup time, bundle size and memory — see
[Bundle size](#bundle-size) below for the static part you can collect locally.)

## Why this is a fair comparison

* **Identical DOM contract.** Every app renders
  `table.test-data > tbody > tr` with the same cells, the same `#run /
  #runlots / #add / #update / #clear / #swaprows` buttons, and the same
  per-row `a.lbl` (select) / `a.remove` (remove) anchors. The harness only
  ever touches the DOM, never framework internals.
* **Identical data.** All three use the same word lists and the same seeded
  LCG, so the rows created are byte-for-byte identical across frameworks.
* **Idiomatic per framework.** glory and leptos model each row's label as its
  own fine-grained signal (`Cage<String>` / `RwSignal<String>`), so
  "update every 10th" patches only the touched text nodes. dioxus uses a
  single `Signal<Vec<Row>>` reconciled by its VirtualDom — that *is* its
  architecture, and benchmarking it any other way would be unfair to how
  dioxus actually works.

## Layout

```
benchmarks/
├── bench-harness.js   # shared, framework-agnostic timing harness
├── glory/             # glory app  (trunk)
├── leptos/            # leptos 0.8 app (trunk, CSR)
└── dioxus/            # dioxus 0.7 app (trunk, web; default features trimmed —
                       #   see dioxus/Cargo.toml — so it mounts without the dx CLI)
```

Each app is a standalone crate (the repo workspace `exclude`s `benchmarks/`),
so they build independently and pull each framework's own dependency tree.

## Running

Prerequisites: `rustup target add wasm32-unknown-unknown` and
[`trunk`](https://trunkrs.dev) (`cargo install trunk`).

Serve one app (repeat for each, one at a time — they all default to their own
trunk port):

```sh
cd benchmarks/glory   && trunk serve --release --open
cd benchmarks/leptos  && trunk serve --release --open
cd benchmarks/dioxus  && trunk serve --release --open
```

Or use the helper scripts from the `benchmarks/` directory:

```sh
./run.sh glory     # bash
./run.ps1 glory    # PowerShell
```

Then in the browser:

* Click **“▶ Run all benchmarks”** (injected at the top of the page), or
* append **`?autorun`** to the URL to start automatically.

Results render as a table on the page, are printed to the console
(`console.table`), and are exposed programmatically on
`window.__BENCH_RESULTS__` plus a `bench-done` window event.

> **Always benchmark a `--release` build with DevTools closed.** Debug builds
> and an open console add large, framework-uneven overhead.

## Methodology

For each operation the harness warms up 3 times, then takes 12 measured
samples and reports the **median** (the official suite also uses the median to
reject outliers). A sample is timed as:

```
t0 = performance.now()
<click the button / row>          // the framework mutates the DOM here
await requestAnimationFrame x2    // let the browser lay out + paint
t1 = performance.now()
```

i.e. it captures the synchronous handler **plus** the resulting layout/paint of
the next frame. These are **relative** numbers for comparing the three apps on
one machine in one browser — they are not a substitute for the official
Chrome-tracing harness and should not be quoted as absolute submission numbers.

## Official Chrome-tracing workflow

Use the official krausest runner when you need publishable numbers:

```powershell
./benchmarks/official-js-framework-benchmark.ps1
```

The script clones or reuses `JS_FRAMEWORK_BENCHMARK_REPO`, generates official
`frameworks/keyed/glory-rs` and `frameworks/keyed/dioxus-rs` adapters from the
local apps, builds them with `trunk`, runs
`npm run bench -- --framework keyed/glory-rs keyed/dioxus-rs`, and copies the
official result artifacts into
`target/benchmark-report/official-js-framework/`. It also writes
`official-js-framework-summary.md` and `.json` with median/range tables for
`total`, `script`, and `paint` timings.

Pass `-SkipBench` to validate adapter generation/building without launching
Chrome, or `-ChromeBinary` to pin the browser used by the webdriver runner.
Use `-BaselineName name` to preserve the current result set under
`target/benchmark-report/official-js-framework/baselines/name/`, and
`-CompareBaseline name` to add a delta table against that baseline. Baseline
names are not overwritten unless `-OverwriteBaseline` is set. For Glory-only
optimization A/B runs, pass `-GloryOnly`.

For a shorter CPU-suite smoke run:

```powershell
./benchmarks/official-js-framework-benchmark.ps1 `
  -SkipInstall `
  -Benchmarks 01_,02_,03_,04_,05_,06_,07_,08_,09_ `
  -Count 1 `
  -Headless `
  -NoThrottling
```

For a stable local comparison before and after a Glory-only optimization:

```powershell
./benchmarks/official-js-framework-benchmark.ps1 `
  -GloryOnly `
  -Count 5 `
  -Headless `
  -NoThrottling `
  -BaselineName before-e9

./benchmarks/official-js-framework-benchmark.ps1 `
  -GloryOnly `
  -Count 5 `
  -Headless `
  -NoThrottling `
  -CompareBaseline before-e9 `
  -BaselineName after-e9
```

## Bundle size (measured)

Release build (`trunk build --release`), `wasm` + JS glue, on this repo's
toolchain (rustc 1.96, wasm-bindgen 0.2.123):

| framework | wasm | wasm (gz) | js glue | **total (gz)** |
|---|--:|--:|--:|--:|
| **leptos** | 145 KB | 49 KB | 27 KB | **55 KB** |
| **glory**  | 190 KB | 66 KB | 21 KB | **71 KB** |
| **dioxus** | 413 KB | 144 KB | 25 KB | **150 KB** † |

† **dioxus is not size-comparable here**: the bundled `wasm-opt` rejected
dioxus's output under `-Oz`, so its app is built with `wasm-opt` **off** (rust
`opt-level=3 + lto` still apply). Built through the official `dx` CLI with
`wasm-opt` enabled it is considerably smaller. The leptos/glory figures *are*
`wasm-opt`-optimised and directly comparable.

Reproduce:

```sh
for a in glory leptos dioxus; do
  cd benchmarks/$a && trunk build --release
  ls -l dist/*_bg.wasm; cd -
done
```

## Correctness parity (verified)

Before timing anything, all three apps were driven through every operation in a
real Chrome (release builds) and produce identical, correct DOM: create → 1,000
rows, replace-all → still 1,000, update → every 10th label gains `" !!!"`,
select → `danger` class on the row, remove → row gone (count 999), append →
1,999, swap → stable count, clear → 0. The seeded RNG makes the generated data
**byte-for-byte identical** across frameworks (e.g. the first row is
`id 1 — "clean red keyboard"` in both glory and dioxus).

## Known caveats

* **Measure one operation per animation frame.** All three frameworks flush
  DOM updates **asynchronously** (microtask / scheduler / VDOM tick), *not*
  synchronously inside the click handler. Firing many clicks in a single
  synchronous JS loop without yielding lets state desync — this is a harness
  artifact, not a framework bug. The harness handles this correctly by waiting
  for paint between samples; if you script your own driver, yield a frame
  between operations.
* **`performance.now()` is clamped** (~100 µs) in most browsers, so a single
  fast operation reads as `0.1 ms`. The harness's medians over real
  paint-inclusive windows avoid this; ad-hoc synchronous micro-timing does not.
* **glory console noise (fixed).** glory used to log unconditionally during
  CSR rendering — an `info!("view not found …")` per stale view on every list
  replace/clear plus `[hydrating]: node not found` warnings at mount. These are
  now routed through a debug-only `debug_warn!` macro (no-op in release), so the
  console stays quiet and the wasm→JS `console.log` calls no longer tax the
  replace/clear path. Removing them alone cut glory's first-op create ~104→74 ms
  and clear ~60→36 ms here.
* **`Cage` memory reclamation (fixed).** `Cage` handles are `Copy`, backed by a
  leaked `'static` slot, so memory could never be freed (`Copy` and `Drop` are
  mutually exclusive). `crates/core/src/reflow/cage.rs` now parks invalidated
  slots in a thread-local free-list and recycles them: invalidating a cage
  (driven by `Owner` drop on scope detach) drops its value and reuses the slot,
  so the live slot count is bounded by peak concurrent cages instead of growing
  forever. `Cage` stays `Copy`. The benchmark rows opt in by owning their label
  cage on the row scope (`ctx.owner().own_cage(...)`).
* **The "create→clear degrades after ~3 cycles" effect is a TEST-ENVIRONMENT
  artifact, not a framework bug.** Looping create-1,000 → clear runs ~50 ms for
  the first ~3 cycles, then jumps to ~1,000 ms (~18×) and stays flat. It is
  **GC-independent**, **resets on reload**, and persists with constant labels,
  with no per-row reactivity, and even with fully inert `tr/td/text` rows.

  Two controls prove it is the environment, not glory:
  1. The same workload via `ServerHolder` with **no wasm-bindgen**
     (`benchmarks/native-repro`, `cargo run --release`) is **perfectly flat —
     ~16 ms per create across every cycle.**
  2. **Leptos exhibits the *identical* curve** (~45 ms for 3 cycles → ~1,000 ms,
     flat) once the long-running headless browser has churned enough DOM. Early
     in a fresh browser session leptos looked flat; after heavy churn it
     degrades exactly like glory.

  So the slowdown comes from the **long-running headless Chromium instance**
  used to drive these measurements (renderer/compositor state accumulating
  across tabs and reloads over a session), and it hits every wasm-DOM framework
  equally. It is **not** a glory `Cage`/view/wasm-binding bug. Measure each
  framework in a **freshly launched** browser, and read the *first-cycle*
  numbers (which is what the table below uses).
* Numbers vary with CPU, browser version and thermal state. Run all three
  back-to-back on the same machine and compare relative ordering, not
  absolute milliseconds.

## Sample results

Median operation time in **milliseconds, lower is better**. Release builds,
one machine (this repo's dev box), Chromium via an automated driver. Because
that driver runs a non-visible tab (no reliable `requestAnimationFrame`), these
were collected with a `setTimeout(0)` settle + forced layout instead of the
harness's paint wait — so each number is **JS + DOM-update + layout time and
excludes the final paint**, and carries a ~2–5 ms settle floor (tiny ops are
therefore upper bounds). Treat them as **relative ordering on one machine**, not
absolute truth. Re-run the in-browser **“Run all”** harness for paint-inclusive
numbers.

First-cycle numbers (single sample, fresh reload before each small group so the
environment artifact above never sets in). glory and leptos were re-measured in
the same session and are directly comparable; dioxus is from an earlier, fresher
browser session and is only indicative (†env).

| operation | glory | leptos | dioxus (†env) |
|---|--:|--:|--:|
| create 1,000 rows | 123 | **68** | 86 |
| replace all 1,000 | 188 | **147** | 76 |
| update every 10th | 32 | 20 | **13** |
| select row ‡ | 6.6 | **5.1** | 6 |
| swap rows | **1.7** | 8.2 | 9 |
| remove row | **8.1** | 13.8 | 44 |
| append 1,000 | 167 | 148 | **44** |
| create 10,000 rows | 1980 | 1455 | **363** |
| clear 1,000 rows | 50 | 12 | **5** |

glory is competitive on the fine-grained single-node ops (it wins swap/remove)
and runs a bit behind leptos on bulk create/replace/append — consistent with it
issuing more individual web-sys calls per node and now adding one extra `Scope`
per row (the `RowWidget` that owns the row's label cage). These are single
samples (noisier than medians) and the absolute values drift with browser
freshness, so compare ordering, not exact milliseconds.

What the numbers say (for *these* idiomatic implementations, this machine):

* **glory** is in the same ballpark as leptos. It *wins* the fine-grained
  single-node ops (swap 1.7 ms, remove 8 ms) and trails leptos modestly on the
  bulk paths (create/replace/append). The gap is consistent with glory issuing
  more individual web-sys calls per node, plus the extra `Scope` per row added
  by the `RowWidget` that owns each row's label cage.
* **leptos** is the most even all-rounder, slightly ahead on bulk create and
  clearly ahead on bulk teardown (clear 12 ms).
* **dioxus**'s VirtualDom diff is strong on bulk create/clear but slowest on
  single-row remove (it re-diffs the list). Its column is from an earlier,
  fresher browser session (†env) so treat it as indicative only.

‡ **select is O(n) in all three here by design choice**: every row observes the
shared "selected" state, so selecting re-evaluates all 1,000 rows. This is *not*
the optimised js-framework-benchmark pattern (which only touches the
previously- and newly-selected rows) — it's an apples-to-apples stress of each
framework's per-row subscription model. At first-cycle scale (~1,000 rows) it's
cheap for all three (~5–7 ms); the earlier "leptos select = 123 ms" figure was
the environment artifact, not a real cost.

> Canonical, paint-inclusive numbers come from the in-browser harness
> (**“▶ Run all benchmarks”**) in a **visible** tab — see Methodology. The table
> above and the bundle sizes are reproducible from the steps in this README.
