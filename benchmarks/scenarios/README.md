# Portfolio update scenarios

This independent suite complements semantic-rows-v1; it does not replace or
change the existing bulk benchmark. All 1,000 / 5,000 positions stay mounted.

- Glory uses handwritten Rust Plan, live Each item Cages and direct field bindings.
- Dioxus root uses one collection Signal and keyed row templates.
- Dioxus row uses keyed row components, a local Signal per position and an
  independent total component. Registry prop equality compares Rc identity.

All three run the same Rust Market controller and integer price/quantity/amount
calculations. Updates contain one position, 1%, 10%, all positions, or ten 1%
market ticks delivered in one event. Selection updates the old and new row only.
An index registry gives both granular implementations direct access; neither
scans the source to find a changed position. Initialization and close are timed
separately. The market and UI copies are present in all variants.

The public builders/signals are used without modifying any framework. The row
version is necessary to distinguish component granularity from framework costs.
Shared business work and DOM shape are frozen; framework metadata remains.

```powershell
# Fresh independent release targets, wasm Clippy, data/identity/geometry/pixels,
# two opposite-order rounds; requires the repository Playwright installation.
./benchmarks/scenarios/run.ps1
```

The existing wasm-bindgen 0.2.129 CLI must be installed at
`D:/BuildCache/glory-framework-compare/tools/bin/wasm-bindgen.exe`.
run.ps1 uses D: for Cargo output, TEMP/TMP, Python cache, browser profiles,
downloads and evidence. It owns and closes a loopback server and browser.
The affinity helper assumes this machine's 16 P-core logical processors
(mask 65535); adjust explicitly before comparing on another CPU.

Each variant/size/round gets a fresh context. Each operation has three warmups
and twelve samples. Persistent updates do not include re-creation; each operation
starts from a fresh portfolio. Mount/close include their respective UI work.
CDP Script/Task/Layout/Style counters exclude the independent JS oracle, complete
DOM/Text identity checks, stats, hashing and screenshots. Wall is click to two
RAF callbacks, not presentation latency or FPS. There is no DOM monkey patch or
CPU profiler in the timed run.

The oracle checks every price, quantity, amount, class and portfolio total after
every operation. Persistent updates preserve all row Element/Text objects and
Glory identity attributes. Acceptance compares every row/cell box, four PNG
checkpoints, and live registries after closing. Internal Dioxus graph/JS heap
release is not claimed by these checks.
