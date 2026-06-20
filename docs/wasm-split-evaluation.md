# Wasm Split Evaluation

Date: 2026-06-12

This note closes the C3 evaluation from `_todos.md`.

## Current Baseline

The repository already tracks representative wasm size with:

```powershell
./benchmarks/wasm-size.ps1
```

The 2026-06-11 raw release results in `docs/performance.md` were:

- minimal `_test-size`: 1.24 MiB raw release wasm;
- `counter`: 1.29 MiB raw release wasm;
- `router-basic`: 1.49 MiB raw release wasm;
- `todomvc-fullstack`: 1.61 MiB raw release wasm.

The frontend benchmark apps also track final `wasm` + JS glue gzip size in
`benchmarks/README.md`; Glory was 71 KiB total gzip in that local release build.

## Decision

Do not implement wasm splitting now.

Glory's current examples are still small enough that wasm splitting would add
more toolchain complexity than user value. The more useful near-term work is:

- keep measuring raw and final compressed wasm size;
- improve asset hashing and bundling;
- profile `create10k` and DOM call count before adding a chunking system;
- keep route-level and feature-level code organization simple until an app has
  enough payload to benefit from lazy chunks.

## Hot Reload Interaction

A wasm-split implementation would have to coordinate with:

- the CLI watcher and rebuild target graph;
- `wasm-bindgen` output naming and generated JS glue imports;
- `wasm-opt` and compression for every emitted chunk;
- hot reload message routing, because a changed split module may need either a
  targeted reload or a full webview reload;
- desktop/mobile asset serving, where chunk URLs must resolve from the bundle
  root as well as from `glory serve`.

Until chunk boundaries are stable, this would make development reloads less
predictable.

## Revisit Triggers

Reopen wasm splitting when at least one of these is true:

- a real Glory app exceeds 250 KiB gzip for initial `wasm` + JS glue after
  `wasm-opt -Oz`;
- a route-heavy app can identify a lazy chunk that saves at least 100 KiB gzip
  from first load;
- benchmark/report output shows wasm size growth above 20% for a representative
  example after a feature lands;
- users need route-level lazy loading for a production app.

The first prototype should be CLI-owned and route/module agnostic: build named
secondary wasm entrypoints, emit them into the site package directory with
hashed filenames, and let app code load them explicitly. A macro-level
`#[wasm_split]` equivalent should wait until that simpler pipeline works.
