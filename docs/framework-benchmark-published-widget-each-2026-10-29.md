# Published Widget and Each benchmark — 2026-10-29

This run measures the clean, published Glory commit [dae6f2c](https://github.com/glory-rs/glory/commit/dae6f2cc573df94a56bbc9a1e7a1a711a56c1c92), including unified Widget/DSL authoring, source-generic Each, and mixed-tree renderer support. It uses the existing `benchmarks/glory` XML frontend and the shared `semantic-rows-v1` div-row workload.

All pending implementation changes were committed and pushed before measurement: Glory `dae6f2c`, Sapid `722a9da`, and website `a7f7efb`. The report is a separate documentation commit.

## Method

- Pinned peers: Leptos 0.8.22 and Dioxus 0.7.10, using their committed benchmark manifests and lockfiles.
- Intel Core Ultra 9 285H, 16 cores / 16 logical processors; Windows; Chrome 155.0.8059.86.
- Rust 1.98.0 (`88d9e12ae`), Cargo 1.98.0; wasm32 release builds, opt-level 3, LTO, one codegen unit. No additional wasm-opt pass.
- Fresh independent Chrome process per framework run, headless, GPU disabled, 1280×900, scale 1, en-US, light mode; owned browser processes use the same CPU affinity mask 65535 and AboveNormal priority.
- Four rounds alternate `glory, leptos, dioxus` and the reverse order. Each operation has three untimed warmups and twelve measured samples per round.
- **1,296 accepted samples:** 48 per operation per framework. All slow samples remain in the evidence. Tables show the median across all 48 samples.
- The existing acceptance, descendant-integrity and measurement functions are unchanged. A saved driver adapter supplies current-commit metadata and the three built site routes.
- Full seeded-data hashes, retained descendant identity, real 10,000-row rectangles and six matching screenshots were checked before timing.
- One final Glory framework run was rejected because unrelated Rust builds started during it. The eleven complete framework runs were retained; after compilation was quiet, that entire missing framework run was repeated. The interrupted evidence and retry driver are preserved.

## Script CPU time

CDP `ScriptDuration`, milliseconds; lower is faster. These values exclude setup and the subsequent untimed data checks.

| Operation | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| Create 1,000 | 6.285 | 15.465 | 4.678 |
| Replace 1,000 | 10.267 | 20.547 | 8.948 |
| Update every 10th | 0.536 | 0.590 | 1.148 |
| Select row | 0.326 | 179.391 | 1.016 |
| Swap rows | 0.402 | 0.244 | 1.161 |
| Remove row | 0.385 | 0.521 | 1.064 |
| Append 1,000 | 5.705 | 14.574 | 6.392 |
| Create 10,000 | 59.843 | 163.200 | 42.001 |
| Clear 1,000 | 4.865 | 20.754 | 3.312 |

Leptos selection is a fixture-specific anomaly: its Script median is 179.391 ms, with per-round medians of 178.969 / 179.776 / 177.022 / 179.641 ms. It passed the same data/DOM oracle, but this result is unsuitable for a general framework selection-speed claim. Earlier reports also recorded unusual selection behavior in this sequential fixture; this run does not establish its cause. The samples were retained unchanged.

## Wall time

Click to two RAF callbacks, milliseconds. This includes scheduling and frame waits; it is not measured pixel presentation latency or FPS.

| Operation | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| Create 1,000 | 39.90 | 43.60 | 40.90 |
| Replace 1,000 | 38.20 | 44.75 | 37.75 |
| Update every 10th | 19.70 | 20.55 | 23.05 |
| Select row | 23.10 | 181.15 | 22.15 |
| Swap rows | 23.60 | 22.70 | 22.30 |
| Remove row | 22.80 | 21.15 | 21.25 |
| Append 1,000 | 37.50 | 42.15 | 42.00 |
| Create 10,000 | 319.50 | 434.00 | 290.55 |
| Clear 1,000 | 20.90 | 25.15 | 21.65 |

## Artifacts and live state

Raw generated WASM and JS bytes, with the fixtures’ existing feature selections:

| Artifact | Glory | Leptos | Dioxus |
|---|---:|---:|---:|
| WASM bytes | 2,040,614 | 278,790 | 456,622 |
| JS bytes | 79,887 | 30,134 | 51,608 |

Glory’s 10,000-row census has 10,001 instances/records, 50,007 targets, 30,005 graph nodes and 10,002 Owners. Physical census: 80,008 elements, 30,006 text nodes and one comment. Cleanup and retained-key identity checks passed.

Glory uses less Script CPU than this Leptos fixture on creation, replacement, updates, removal, append and clear; Leptos swaps faster. Dioxus creates/replaces/clears faster, while Glory uses less Script CPU for updates, selection, swaps, removal and append. Timing results describe these fixtures and this machine, not all applications.

## Source and evidence

All frozen source files were SHA-256 verified after the completed run. Original sources, lockfiles, per-framework Cargo metadata, build/Clippy logs, artifact hashes, screenshots, environment samples and every accepted timing sample are under:

`D:/BuildCache/glory-published-benchmark-20261029/`

- `source/inputs.json`: clean published commit and complete frozen input hashes.
- `build/`: independent framework targets, release artifacts, metadata and logs.
- `run1/evidence.json`: complete four-round results and correctness evidence.
- `run1/summary.json`: medians, minimum/maximum and per-round medians for wall, Script, Layout, RecalcStyle and Task CPU counters.
- `run1/evidence.interrupted.json`: eleven retained framework runs and the concurrent-build rejection.
- `run1/driver-source/`: exact original and resume drivers, shared checks and timed measurement code.
- `run1/affinity.jsonl`: owned browser CPU affinity and priority records.

The build uses `benchmarks/build.ps1 -Apps glory,leptos,dioxus` against the frozen source. `run-published.cjs` derives its routing/metadata adapter from `benchmarks/compare.cjs`; `resume-published.cjs` reuses its unchanged measurement helper for the rejected complete framework run. Both drivers and the summary converter are saved alongside the evidence.

The independent standard HTML-table js-framework-benchmark suite has a different workload and measurement method. Its scores, native layout/painting, OS input behavior and RSS are outside this run.
