# Glory / Leptos / Dioxus: current semantic browser benchmark

最新三框架独立重跑见 [2026-10-25 最新稳定版对照](../docs/framework-benchmark-latest-stable-2026-10-25.md)：
Glory 当前源码、**Leptos 0.8.22 / Dioxus 0.7.10**（registry 稳定版复核），三个全新 target。
相同九项、正反两轮 **648 个**正式样本，数据、完整后代身份、万行几何及六处截图一致。
Glory / Leptos / Dioxus 的千行创建 Script 为 **19.391 / 13.241 / 4.388 ms**，
替换千行为 **29.980 / 20.257 / 7.548 ms**，万行为 **230.614 / 168.170 / 44.815 ms**。
报告保留 Leptos 连续流程选行异常，另列 72 个独立诊断样本和正常选行结果，不混入主表。

上轮实测见 [HTML / DOM 映射与桥接优化报告](../docs/html-dom-bridge-optimization-2026-10-25.md)：
10 份独立性能构建，首批 1,296 / 最终 1,512 个正式样本，分批统计。
保留版千行创建 / 替换 / 万行创建 Script 为 **19.896 / 29.855 / 230.928 ms**，
相对同批 before 改善 **10.3% / 7.6% / 6.2%**；完整 DOM 写入、两份 Memory 和节点身份保留。
原型寻址每行 19→10 次物理边访问，直接 Node 句柄 27,070→11,054。
报告含 Leptos/Dioxus 全九项、双向桥接计数、撤回 Vec/数字对/栈缓冲实验，
以及当前工作区额外独立构建与完整回归（903/902、27 组浏览器、64/1024 真实行高）。
主 A/B 的 15 个应用输入不变；当前 API 命名迁移作为另一源集仅核验正确性。

上轮实测见 [核心响应图与提交优化报告](../docs/reactive-runtime-optimization-2026-10-24.md)（2026-10-24 测量、10-25 复核）：
逐项消融 **1,944 个**主样本，最终同批对照另 **1,296 个**，不混池。
默认 Glory 千行创建 / 替换 / 万行创建 / 清空的 Script 为
**20.975 / 30.986 / 247.109 / 15.317 ms**；选行约快 30%，替换约快 5%，
批量创建收益不稳定。保留两份 Memory、完整身份、真实几何和逐像素检查。
报告同时记录原生高扇出、WASM Node 体积增加、空帧/多依赖回退、缓冲保留成本和撤回实验。
标准 benchmark 继续用原来的 Bond/Lotus，直接闭包 API 作为独立迁移对照。

上轮实测见 [2026-10-24 双份 Memory 声明共享报告](../docs/shared-declarations-2026-10-24.md)：
六份全新独立构建、复制声明/共享声明/内联消融对照，最终 **1,296 个**正式样本。
Glory 千行创建 / 替换 / 万行创建 / 清空的 Script 为
**19.59 / 30.43 / 234.37 / 15.45 ms**，相对同批 before 降低约 16% / 17% / 13% / 12%。
同批 Leptos 为 15.30 / 22.86 / 187.55 / 17.59 ms，
Dioxus 为 4.52 / 7.85 / 60.93 / 8.96 ms。全部数据、存续节点身份、万行几何和画面一致；
报告保留消融慢样本、绑核辅助脚本中断/恢复、完整回归和未获益项，不合并不同源码批次。

上一轮优化实测见 [2026-10-24 节点存储与 DOM 元数据报告](../docs/node-storage-dom-metadata-2026-10-24.md)：
五个全新独立 target，Memory-only 拆分对照，四轮 **2,160 个**正式样本。
Glory 千行创建 / 替换 / 万行创建 / 清空的 Script 为
**23.90 / 36.87 / 256.21 / 16.86 ms**，相对本轮 before 降低约 14% / 20% / 25% / 31%。
双份 Memory、完整节点身份和全部 DOM 属性仍保留。报告包含三框架全表、
未获益的小操作、首轮慢样本与补跑、源码边界及完整回归证据。

上一轮三框架实测见 [2026-10-24 静态构造计划后对照](../docs/framework-benchmark-static-plan-2026-10-24.md)：
三个全新独立 target、相同九项、正反顺序 648 样本。Glory / Leptos / Dioxus
千行创建 Script 为 **28.47 / 14.21 / 4.46 ms**，万行总耗时为
**620.65 / 437.40 / 298.25 ms**。数据、存续节点身份、完整万行几何和 18 张画面检查点一致。
完整主表保留选行的运行历史效应，并附 72 个独立诊断样本；本轮未改实现或 workload。

静态计划实现及同库 before/after 见 [静态构造计划报告](../docs/static-construction-plan-2026-10-24.md)。

此前优化及源码分析见 [2026-10-24 创建提交/Dioxus 报告](../docs/bulk-create-dioxus-2026-10-24.md)：
独立构建、相同九项、正反顺序 1,080 样本；保留的 Glory 创建桥接将万行 Script 从
702.08 降到 654.75 ms，当前三框架万行总耗时为 909.45 / 465.10 / 313.70 ms。
历史结果和未保留实验均有明确边界，主计时 workload/harness 未改。

The previous [three-framework rerun after DOM creation optimization](../docs/performance.md#three-framework-rerun-after-dom-creation-optimization--2026-10-23)
freshly builds all three from a frozen 251-input snapshot and measures
648 samples in opposite order under the same P-core affinity. Create-10k wall
medians are 955.10 / 439.20 / 297.35 ms for Glory / Leptos / Dioxus.
The report includes all nine operations, CPU counters, per-round variation,
independent selection diagnostics and exact source/identity/geometry/pixel evidence.

The earlier [HTML mapping/DOM creation optimization](../docs/performance.md#html-mapping-and-dom-creation--2026-10-23)
compares fresh independent Glory before/after builds with 432 unchanged-workload
samples. Script medians improve 17.0% for create-1k, 7.8% for replace-1k and
11.4% for create-10k. Final attributes/identities remain intact: finite native
prototypes reduce replacement setAttribute calls from 32k to 15k and UTF-8
decodes from 78,006 to 34,006. The report records 25 browser suites, complete
native regression, real-height cache checks, and the concurrent-source boundary.

The previous [three-framework rerun after teardown optimizations](../docs/performance.md#three-framework-rerun-after-teardown-optimizations--2026-10-23)
freshly builds Glory, Leptos 0.8.19 and Dioxus 0.7.9 independently from a frozen
243-input snapshot. Two opposite-order rounds produce 648 samples, with the
same P-core affinity for all three. Median create-10k wall time is
1012.15 / 451.30 / 306.95 ms; clear-1k is 33.70 / 19.25 / 9.35 ms.
Data, surviving DOM/Text identities, all 10k row rectangles and six viewport
checkpoints agree. The report includes CPU counters, per-round variation,
Leptos's history-sensitive selection diagnostic and the source boundary for
concurrent changes. The host also ran other project builds; this is a local run.

The earlier [Scope/unlink follow-up](../docs/performance.md#scope-cleanup-and-sibling-unlink--2026-10-23)
compares independent builds under the same P-core affinity: 432 primary samples,
with 1,296 earlier default-scheduling samples retained separately. Clear-1k script
median improves 27.5%; replacement improves only 1.6%. See the report for variance,
node/pixel checks and full regression.

The [Owner/Binding retirement follow-up](../docs/performance.md#owner-and-binding-retirement-follow-up--2026-10-23)
records the next 432-sample paired run, identity/pixel checks and full regression.
Clear-1k script median improves 6.7%; replacement has no demonstrated gain.

The subsequent [DOM retirement follow-up](../docs/performance.md#dom-retirement-follow-up--2026-10-23)
compares fresh Glory before/after builds with 432 unchanged-workload samples,
removes repeated parent-table scans, and records complete regression evidence.

The previous rerun independently rebuilt all three apps and measured two
opposite-order rounds (648 samples). Its historical table, CPU counters, untimed
node census, selection diagnostic and immutable evidence are in
[Performance Notes](../docs/performance.md#current-three-framework-rerun--2026-10-23).
The earlier one-round table below remains historical.

The [bulk DOM follow-up](../docs/bulk-dom-optimization-2026-10-23.md) compares
Glory before/after framework optimizations with this same unchanged workload:
432 samples, reversed-order rounds, independent builds and identity/pixel checks.
Its paired results and the then-existing adaptive anchor failure are recorded
separately from the earlier three-framework table below. The anchor failure was
subsequently [fixed](../docs/anchor-residual-fix-2026-10-23.md). Use `bulk-compare.cjs`
and `summarize-bulk.py` for the paired run; the report includes exact commands.

The active workload is **semantic-rows-v1**. It compares Glory's new public Rust
builders and full keyed `each` with Leptos 0.8.19 and Dioxus 0.7.9. All three
use the same seeded Rust generator, div/button/span element structure and CSS.
Framework-specific attributes and comment markers remain measured overhead.
The operations are create 1k/10k, replace, update every tenth label, select,
swap, remove, append and clear. Every row is mounted; this is not a virtual list.

These are **local comparison results**, not official HTML-table scores. The
official adapter script is explicitly disabled for this changed DOM contract.
Historical official tables below do not describe the current architecture.

## Run

Use PowerShell on Windows with Rust's `wasm32-unknown-unknown` target, Node and
the Playwright package under `tests/playwright/node_modules` installed. Supply
wasm-bindgen-cli **0.2.129** at
`D:/BuildCache/glory-framework-compare/tools/bin/wasm-bindgen.exe`.
The framework library versions and bindgen cohort are locked in the three
checked-in Cargo.lock files; the CLI version must match.

```powershell
./benchmarks/compare.ps1
```

This freezes the current owned crate/app inputs with SHA-256, builds each app
in an independent D: Cargo target (release opt-level=3/LTO/one codegen unit),
runs strict WASM Clippy, generates web bindings without wasm-opt, starts a
loopback server, launches an independent headless Chrome and performs semantic
conformance before timing. It closes its browser and owned server on exit.
The default browser is installed Google Chrome; `-Browser` selects an executable.
All targets, temporary browser profiles and results must stay under D:/BuildCache.

```powershell
# Reuse already built sites; each run needs a new output directory.
./benchmarks/compare.ps1 -SkipBuild `
  -OutDir D:/BuildCache/glory-framework-compare/my-round
```

To check order sensitivity, reuse one build and run a separate reversed round:

```powershell
./benchmarks/compare.ps1 -SkipBuild -Order 'leptos,glory,dioxus' -OutDir D:/BuildCache/glory-framework-compare/forward-round
./benchmarks/compare.ps1 -SkipBuild -Order 'dioxus,glory,leptos' -OutDir D:/BuildCache/glory-framework-compare/reverse-round
```

For a one-app preview use `./benchmarks/run.ps1 glory` and visit the printed URL.
The module exposes `window.__BENCH__.runAll()`; structured three-app measurements
should use compare.ps1 so the CPU counters and conformance checks are included.
The old Trunk cache/output path is no longer used by the supported runner.

## What is validated and timed

- An independent JS seed oracle verifies row IDs/labels and all nine operations.
- Swap must preserve keyed DOM identities. Every one of the 10k rows has a box.
- Clear/recreate checks that Glory's live label registry and graph return to
  baseline. Other frameworks' internal reactive graphs are not inspected.
- Same-checkpoint viewport PNGs are byte-identical; final normalized row-data
  hashes agree for every timed sample. This is not a claim about every pixel
  in every possible state or browser.
- Each operation warms 3 times and measures 12 samples. Setup and row-data
  checks are outside the click-to-two-RAF interval. This is not presentation/FPS.
- CDP script/layout/style/task CPU counter deltas are recorded as diagnostics;
  they include common harness overhead. Inclusive metrics must not be summed.

## First current-architecture round: 2026-10-23

Chrome 155.0.8059.84, 1280×900, 324 measured samples. Wall medians in ms:

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

Glory currently trails substantially on bulk mount/replacement/append/clear;
create-10k is about 2.81 s versus 0.51/0.55 s. Its script time is the largest
gap. Small updates are much closer. The Leptos mixed selection result is
history-sensitive; fresh-list diagnostics do not reproduce it as a general
selection baseline. There is no overall ranking from this one ordered round.

See [current measurements and caveats](../docs/performance.md) for CPU counters,
the separate selection probe, live node census, build details and raw evidence.

<details>
<summary>Historical table workload and previous-architecture measurements</summary>

The material below is retained history. Its old APIs, official-table commands,
bundle settings and results do not describe the semantic-rows-v1 apps above.

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

</details>
