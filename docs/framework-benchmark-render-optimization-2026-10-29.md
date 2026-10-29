# Display creation, replacement, clearing and artifact retention

This report compares the same `semantic-rows-v1` workload before and after the
display/runtime optimizations. Public authoring still uses one `Widget` and one
`Each` controller for KeyedList and Vary/Vec sources, with the same keyed row
ownership, field aliases and node reuse. No benchmark application, peer source,
feature selection, stylesheet or operation was simplified.

## Evidence and controls

All artifacts are under `D:/BuildCache/glory-render-opt-20261029/`:

- `before/inputs` freezes the original 555 inputs at `4011c68`. Every input hash
  matches the published `dae6f2c` snapshot. `before/site` contains byte-identical
  copies of its three independently built release artifacts; their original
  compiler logs and targets remain in `glory-published-benchmark-20261029/build`.
- `final/inputs` freezes the optimized 556 inputs; `final` retains its independent
  Cargo metadata, strict WASM Clippy output, release compiler JSON and bindgen
  artifacts. The additional runtime source is `core/src/subscribers.rs`.
- `final-run2/evidence.json` retains correctness, screenshots, environment and
  every timing sample. `summary.json` retains all medians, ranges and per-round
  medians. Profiling and coverage are separate from these timing intervals.
- `final-profiles2` retains Chrome CPU profiles and timeline traces;
  `submit-counts-final/evidence.json` records exact JS bridge counts using V8 coverage.
- `final-size2.json` attributes the actual measured artifacts, including function
  bodies, sections, import-reachable JS, SHA-256, gzip and Brotli sizes.
- `probes-acceptance-final/evidence.json`, `unified-evidence.json`, `mixed-check.txt`, `stream-check.txt`,
  `sapid-check.txt`, `core-tests.txt` and `workspace-check.txt` retain verification.

The machine is the same Intel Core Ultra 9 285H Windows host. Rust 1.98.0,
wasm-bindgen 0.2.129, Chrome 155.0.8059.86, Leptos 0.8.22 and Dioxus 0.7.10
remain pinned. All builds use opt-level 3, LTO and one codegen unit; no extra
wasm-opt, feature disabling or JS minification was applied to any timed app.
Four alternating rounds use a fresh browser per framework/round, three warmups
and twelve retained samples per operation. The browser has the same 1280x900
viewport, device scale 1, locale, light theme, CPU affinity and priority. The
runner rejects Cargo/rustc activity at both timing boundaries; measurement starts
only after all checks finish and builds have been absent for thirty seconds.

These are the repository's div-based self-suite results, separate from the
upstream HTML-table js-framework-benchmark. ScriptDuration measures script CPU;
two-RAF wall includes browser scheduling/layout. Neither is a frame-presentation
or process-memory measurement.

## Measured results

All 1,728 samples are retained: 48 per operation/framework. Values below
are medians in milliseconds. Faster-round counts compare the four paired
round medians; they are not confidence intervals.

| Script CPU | Before | Optimized | Change | Faster rounds | Dioxus | Leptos |
|---|---:|---:|---:|---:|---:|---:|
| create_1k | 6.531 | 6.131 | -6.1% | 3/4 | 4.667 | 15.982 |
| replace_1k | 10.358 | 9.222 | -11.0% | 4/4 | 9.321 | 19.815 |
| update_10th | 0.543 | 0.532 | -1.9% | 2/4 | 1.217 | 0.618 |
| select_row | 0.336 | 0.362 | +7.9% | 1/4 | 1.007 | 181.664 |
| swap_rows | 0.430 | 0.418 | -2.7% | 2/4 | 1.215 | 0.245 |
| remove_row | 0.403 | 0.400 | -0.7% | 3/4 | 1.044 | 0.531 |
| append_1k | 5.787 | 4.957 | -14.3% | 4/4 | 6.072 | 15.348 |
| create_10k | 56.168 | 46.854 | -16.6% | 4/4 | 41.710 | 155.241 |
| clear_1k | 5.436 | 4.844 | -10.9% | 4/4 | 3.280 | 20.174 |

Replacement, append, 10k creation and clearing improve in every paired
round. Selection regresses **7.9%**, approximately **0.027 ms**, with only
one faster round. It remains below Dioxus in this fixture, but the
regression is retained. Tiny update/swap/remove differences are not
consistent speedup claims.

Against Dioxus, optimized script CPU is still about 31% slower for
create-1k, 12% slower for create-10k and 48% slower for clear-1k.
Replacement is approximately tied (1.1% lower in this batch). The
fine-grained owner/alias/binding work described above remains, alongside
native DOM cloning and removal. This change narrows the bulk-operation
gap; it does not eliminate all of it.

| Two-RAF wall | Before | Optimized | Change | Dioxus | Leptos |
|---|---:|---:|---:|---:|---:|
| create_1k | 40.85 | 40.70 | -0.4% | 39.35 | 43.60 |
| replace_1k | 37.50 | 37.55 | +0.1% | 36.35 | 44.20 |
| update_10th | 21.75 | 22.35 | +2.8% | 24.10 | 21.05 |
| select_row | 20.70 | 20.90 | +1.0% | 20.30 | 183.40 |
| swap_rows | 23.95 | 20.10 | -16.1% | 23.05 | 21.95 |
| remove_row | 22.65 | 20.95 | -7.5% | 21.95 | 21.50 |
| append_1k | 38.20 | 36.65 | -4.1% | 40.45 | 45.20 |
| create_10k | 313.70 | 283.65 | -9.6% | 298.55 | 393.10 |
| clear_1k | 21.60 | 22.00 | +1.9% | 21.00 | 24.20 |

The 10k wall median improves from 313.70 to 283.65 ms. CPU gains do not
automatically improve every two-RAF latency: clear rises from 21.60 to
22.00 ms, replacement from 37.50 to 37.55 ms, update from 21.75 to
22.35 ms, and selection from 20.70 to 20.90 ms. All are retained.

Leptos selection remains an anomalous 181.664 ms, recurring in all four
rounds and in the previous published run. Its data/geometry/identity
checks pass. The cause was not established here, and this result is not
used to claim a general framework-wide multiplier.

### Artifact bytes

Gzip uses level 9 and Brotli quality 11, summed per imported JS file.
Compression is an offline file metric; the timing server serves the
original files. Custom-section stripping is diagnostic only.

| Bytes | Before | Optimized | Dioxus | Leptos |
|---|---:|---:|---:|---:|
| WASM raw | 2,040,614 | 885,434 | 456,622 | 278,790 |
| WASM gzip | 623,019 | 265,620 | 144,358 | 68,886 |
| WASM Brotli | 450,168 | 199,384 | 112,322 | 52,752 |
| WASM without custom sections | 1,728,945 | 734,445 | 350,355 | 171,864 |
| Imported JS raw | 79,887 | 28,465 | 49,916 | 30,134 |
| Imported JS gzip | 15,053 | 7,422 | 10,769 | 6,094 |
| Imported JS Brotli | 12,803 | 6,429 | 9,381 | 5,309 |
| All emitted JS raw | 79,887 | 35,370 | 51,608 | 30,134 |

WASM decreases **56.6%** and imported JS **64.4%**. Imported JS is now
smaller than Dioxus; WASM is still **1.94x** its raw size, or **2.10x**
after removing custom sections from both. Unimported emitted snippets
are 6,905 bytes in optimized Glory and 1,692 in Dioxus; they are shown
separately rather than counted as entry-module imports.

The WASM code-section payload falls from 1,628,381 to 688,280 bytes,
data from 84,448 to 42,004, and the name payload from 311,388 to 150,708.
Thus the reduction is mostly code, not just metadata. There are 349
baseline function bodies whose names mention `glory_web::browser::`,
including generic instantiations, and zero in the optimized artifact.

Remaining enclosing-symbol buckets include alloc 151,816 bytes,
glory-render 126,841, glory-core 79,510, core 71,465, glory-ui 61,413,
glory-web 58,615 and hashbrown 42,440. These are disjoint function bodies
charged to their enclosing names, including inlined callees; they are
not exclusive crate sizes or independently removable-byte estimates.

The remaining raw-app code still includes Memory.apply (21,657 bytes),
descriptor validation (16,229) and portable UI scope/event construction.
The shared backend enums and portable View build path keep semantic UI
code reachable even after the browser renderer disappears. This points
to static specialization of physical adapters as a further size target,
while retaining one configuration API and one Each controller.

The optimized WASM SHA-256 is
`5a54c684fd92a892dc3b406ec363ae0bc19ff9fee385243c99c07810320385e2`.
All peer hashes, section hashes, sizes and tool hashes are retained in
`final-size2.json` and `provenance.json`. The driver keeps its historic
profile labels; the observed baseline/candidate stats both identify
`html-display-keyed-v2`, as recorded in provenance.

## Hotspots and retained changes

CPU sampling identifies template cloning, target lookup and insertion during
creation; replacement additionally pays retirement, and clearing includes both
DOM removal and reactive/owner teardown. The shared DOM shape remains fully
materialized. Cloning/removal native calls are also visible in Dioxus profiles;
the profile alone cannot assign the entire framework gap to those native calls.
Rust samples also include row construction, `runtime::lookup`, `view::resolve`
and `Instance::set_text`: owner/handle checks and named-slot resolution are
additional per-instance work, not DOM cloning. Those checks remain in place.

There are three additional costs in Glory's path:

1. Each target path formerly accessed `node.childNodes[index]` at every step.
   That crosses the DOM API and exposes live NodeLists repeatedly. Walking
   `firstChild`/`nextSibling` avoids those lists for small offsets while retaining
   shape checks and the same target nodes. Offsets above three retain direct
   indexed access: walking from the first child for every target would become
   quadratic for wide templates. A 200-target acceptance case checks identity,
   writes and a linear bound on sibling getter calls.
2. The bridge formerly capped packets at 256 commands, even when well below
   node/text budgets. The cap is now 4096 commands. The existing 4096-node and
   64-KiB text limits still apply, borrowed WASM views are copied before native
   hooks, and every accepted prefix is accounted separately on failure.
   Creating 10,000 rows now calls `submit` **32 rather than 157 times**; updating
   100 labels remains one call, clearing 1000 rows is three rather than four.
3. Projected row sources commonly have a single subscriber. `Subscribers` keeps
   zero/one edges inline and uses the original sorted BTreeSet for fan-out.
   Promotion/demotion preserves Key iteration order, generations, duplicate
   membership and invalidation scheduling. Event handlers retain the existing
   hashed slot map, including constant-time lookup in event-dense templates and
   dropping arbitrary replacement captures outside the record-table borrow.

The keyed Cache/LIS reconciliation and its reuse rules were not changed. The
instance BindingId list remains intact. Sources are still authoritative
KeyedList members or the existing vector snapshot handles; this introduces no
item mirrors, extra data Owners, second Each or second Widget contract.

Glory's fine-grained row model remains a real cost at membership creation and
retirement: the 10k fixture has 30,005 graph nodes and 10,002 owners. Row label
aliases, label bindings and selection bindings must be registered and retired.
Dioxus's fixture uses a Vec signal and a selection signal and rebuilds keyed
virtual nodes from that Vec inside one App component; it has no Row component
with an equivalent set of per-row reactive handles. These are different update
strategies, even though
data, physical geometry and node reuse are checked against the same oracle.
Graph-node counts are not total-memory measurements of either framework.

## Why the WASM and JS were large

The sparse app could own concrete `WebApp` islands and its generic mount path
could enter type-erased composition before trying the raw View path. Those
references retained the full browser renderer, HTML mapper, input/layer/event
handling and their destruction graph in a raw-template application.

The baseline's largest enclosing function bodies include browser Renderer.apply
(61,357 bytes), HTML semantics.attributes (57,949), events.receive (38,830),
Renderer.drain (30,266), and Renderer.apply's closure (28,559). These methods
were linked even though the timed raw rows do not execute that semantic path.
The optional DSL macro compiler itself is not the source of this runtime size.

Two internal changes make that boundary visible to the linker:

- `Widget::native_host` defaults to None. View advertises its complete sparse
  host path, allowing mounting to skip the type-erased composition/fallback
  branch when its concrete type is known. Box forwards the same hint. Ordinary
  widgets still only need `build`; factories, component tags and manual syntax
  retain the same public authoring API.
- Sparse State stores an internal island interface, with a concrete WebApp
  created only by an actual semantic Widget island. Flush and destruction of an
  otherwise sparse State no longer statically retain the browser renderer.

This is reachability cleanup, not removal of Widget support. Apps that actually
use semantic widgets still include the renderer they need. Opaque dynamic
composition can also keep fallback code reachable. Smaller raw-template output
does not establish size reductions for every Sapid app or full platform parity.

The baseline name section alone is 311,388 bytes. The artifact analysis reports
raw and compressed sizes as well as a diagnostic size with all custom sections
removed, consistently for all frameworks. Timed artifacts are never stripped or
rewritten. JS is reported both as all emitted files (the older benchmark count)
and as the static import graph of app.js, because bindgen can emit snippets that
the entry module never imports.

## Verification

`scripts/check-semantic-ui.ps1` checks the CLI and its native acceptance, engine
fork pins, workspace formatting, all-feature tests, package-only capability
graph/tests, and all-target Clippy. The optimized benchmark separately passes
strict WASM Clippy. Subscriber transitions are compared to an independent
BTreeSet oracle over 4096 deterministic insert/remove steps.

The benchmark verifies seeded payload hashes, every descendant element/text
identity, actual 10k rectangles and six matching screenshots across frameworks.
Acceptance-only probes additionally cover native-hook memory.grow/reentry,
create/place/write/retire failure prefixes, focus/selection for atomic and
fallback moves, record density, teardown and garbage collection of retired DOM.

The mixed-tree checks exercise CSR and real SSR hydration with raw HTML,
Sapid/custom widgets and opaque/concrete factories. They preserve dirty editors,
keyed identity/focus, branch release and mismatch refusal. The real HTTP stream
check additionally covers slot patches, suspension, keyed fallback movement,
End rekey/adoption, composition handoff, malformed packets and cancellation.
Sapid's browser and native mixed-DSL checks also pass. These are the named
browser/offscreen native checks, not a claim about OS IME or screen-reader tests.

## Reproduction

Build the frozen candidate using the same command as the baseline:

```powershell
& benchmarks/build.ps1 -Apps glory `
  -SourceRoot D:/BuildCache/glory-render-opt-20261029/final/inputs `
  -BuildRoot D:/BuildCache/glory-render-opt-20261029/final `
  -Bindgen D:/BuildCache/glory-v2-dioxus-20261028/tools/wasm-bindgen-0.2.129-x86_64-pc-windows-msvc/wasm-bindgen.exe
$env:GLORY_BENCH_BUILD_ROOT='D:/BuildCache/glory-render-opt-20261029'
$env:GLORY_CANDIDATE_SITE='final/site/glory'
$env:GLORY_CANDIDATE_INPUTS='D:/BuildCache/glory-render-opt-20261029/final/inputs/inputs.json'
node benchmarks/compare.cjs new-run 4 before,glory,dioxus,leptos
python benchmarks/summarize.py D:/BuildCache/glory-render-opt-20261029/new-run/evidence.json `
  --out D:/BuildCache/glory-render-opt-20261029/new-run/summary.json
node benchmarks/profile-display.cjs new-profiles before,glory,dioxus
node benchmarks/analyze-display-size.cjs D:/BuildCache/glory-render-opt-20261029/new-size.json `
  before=D:/BuildCache/glory-render-opt-20261029/before/site/glory `
  glory=D:/BuildCache/glory-render-opt-20261029/final/site/glory `
  dioxus=D:/BuildCache/glory-render-opt-20261029/before/site/dioxus `
  leptos=D:/BuildCache/glory-render-opt-20261029/before/site/leptos
```

Earlier trials are retained under trial1/trial2/trial3/trial4. Island erasure
alone plus ordinary inlining did not remove the fallback (2,043,820 bytes), and
forcing mount inlining produced 2,059,479 bytes. Those broad inline changes were
reverted. The first two-round timing trial improved create-10k by 11.5% but
regressed selection by 12.4%; it is diagnostic evidence, not the final result.
The final candidate uses the explicit complete-host hint and bounded runtime
changes described above; the complete four-round comparison is authoritative.
The first complete four-round run remains under `final-run` (its candidate is
`after`). Before accepting that candidate, review found the wide-target sibling
walk and event-vector scaling concerns. The final code bounds sibling walks and
retains the hashed event map; `final-run2` remeasures all four frameworks with
those corrections. No earlier timings were merged into the final medians.
