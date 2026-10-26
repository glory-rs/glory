# Role patch kernel

This standalone Rust fixture measures the actual `Mapper.apply` path, including
prepared validation, commit, edit generation and a counting sink. It does not
measure the DOM, UI graph, frame scheduling, layout or FPS. Its eight cases cover
plain roles, heading level, navigation orientation, grid selection metadata,
long unrelated relations, dialog metadata, equal roles and named default Group.
Setup, warmup and JSON formatting are outside each timed sample. No unsafe
allocator instrumentation or production feature switches are used.

Use `benchmarks/snapshot.py` to preserve before/after inputs, compile both from
the **same absolute source path**, and keep independent target directories. The
fixture source/lock must be identical in both snapshots. Change only the library
files under investigation when staging the after source at that common path.

```powershell
$root = 'D:/BuildCache/glory-role-comparison'
$bindgen = 'D:/BuildCache/glory-website/tools/wasm-bindgen-0.2.129-x86_64-pc-windows-msvc/wasm-bindgen.exe'
# Stage the before snapshot under $root/source first:
./benchmarks/role-patches/build.ps1 -SourceRoot "$root/source" -BuildRoot "$root/before" -Bindgen $bindgen
# Preserve before inputs, then stage after inputs at the same source path:
./benchmarks/role-patches/build.ps1 -SourceRoot "$root/source" -BuildRoot "$root/after" -Bindgen $bindgen
# Keep snapshot manifests at inputs-before/inputs.json and inputs-after/inputs.json.
# Run after all compilation and other tests have finished:
node benchmarks/role-patches/compare.cjs $root
```

`compare.cjs` expects the existing D-drive Binaryen 132.0.0 installation at
`D:/BuildCache/sapid/wasm-size/tools/node_modules/binaryen`, Playwright under
`tests/playwright/node_modules/playwright`, and installed Chrome. Both native
and WASM use opt-level 3, LTO and one codegen unit; WASM additionally uses
wasm-bindgen 0.2.129 and the same Binaryen `-Oz` flags. The script owns its
HTTP server and browser and places temporary files and evidence on D:.

Four rounds reverse before/after order. Every case gets five warmups and twenty
samples per round; each sample performs 2,048 pairs / 4,096 patches. Raw samples,
artifact hashes, environment, medians and interquartile ranges are retained in
`performance.json`. Edit counts and ordered-content checksums must match across
both versions and both execution targets. Existing output cannot be overwritten.
The exact full-shape and independent legacy-rule assertions remain in
`crates/html/src/role_tests.rs`; a checksum is not a substitute for those tests.

The [2026-10-26 report](../../docs/role-patch-lowering-2026-10-26.md) records
correctness, kernel timing, full-application sizes and reproduction boundaries.
