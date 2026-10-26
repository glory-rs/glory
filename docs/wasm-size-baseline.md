# Glory / Sapid WASM size baseline

The [minimal-app attribution](wasm-size-attribution.md) breaks down the retained
code and data after Binaryen, including the regex dependency audit.

Measured on 2026-10-26. The shared, executable matrix lives in
[`benchmarks/wasm-size`](../benchmarks/wasm-size/README.md). It includes 36
applications: an empty WASM, an empty mounted Glory app, text, interactive
Glory/Sapid buttons, optional features enabled without use, and actual use of
each feature group. Sapid is measured against the sibling checkout, so widget
and runtime changes are tested together.

The checked-in [`before.json`](../benchmarks/wasm-size/before.json) preserves
the initial measurements; [`baseline.json`](../benchmarks/wasm-size/baseline.json)
is the optimized regression baseline. Full source archives,
WASM, compressed files, feature trees and browser evidence live under
`D:/BuildCache/sapid/wasm-size/runs/{baseline,optimized}`. The JSON files record
the source digest, dirty revisions, tool versions and artifact hashes; the
archives preserve the exact source bytes rather than relying on Git HEAD.

## Findings

The Glory text app falls from 920,659 to 914,967 B (5,692 B saved); Brotli
falls from 288,913 to 287,806 B (1,107 B saved). The interactive Sapid button
falls from 1,029,679 to 995,151 B (34,528 B / 3.35% saved); Brotli falls
from 319,564 to 310,071 B (9,493 B / 2.97% saved). The empty mounted Glory
application is 894,092 B / 282,146 B Brotli after optimization.

Adding the Sapid dependency without using a widget produces the same WASM as
the Glory text application. All seven Sapid optional features, individually
and together, also disappear when unused in this matrix. Enabling every Sapid
feature around an actual Button has a small binary-size effect; it does not
retain every widget. Feature/dependency counts alone are not payload sizes.

Glory's unused features cause only small binary/compression differences
here: all six combined add 1,966 B raw / 554 B Brotli over the text case.
The Sapid button with all seven unused features adds 260 B raw / 308 B Brotli
over the same button without those features. Actual CSS parsing, hydration,
Fluent messages and resource loaders add code when exercised. The SSR-in-browser and embedded-stream-script probes are
diagnostics: ordinary applications should execute SSR on the server and serve
the streaming controller as its external JS asset.

The empty mounted app still retains much of the finite browser interpreter.
Adding per-widget Cargo features would not remove that shared interpreter.
The measurements support retaining Sapid's existing optional feature groups,
and fixing concrete unwanted code retention before adding more feature gates.

## Changes driven by the measurements

`glory_html::shape` now converts its generic argument at a thin public boundary
and runs the finite mapper in one non-generic implementation. Event payload
validation uses the same pattern. This avoids duplicate implementations for
owned descriptors and borrowed live descriptors without changing validation
or patch behavior. The name-bearing WASM function report identified the
duplicate mapper before the change: two 6,413 / 6,338-byte function bodies
became one 6,676-byte body in the name-bearing wasm-bindgen output. These body
sizes identify duplication; they are not the final compressed savings.

Sapid Button's optional toolbar behavior now lives behind private hooks whose
implementation is constructed only by `in_toolbar`. A standalone button no
longer retains collection registration and roving-focus behavior. Toolbar
setup adds one boxed hook; its public API, focus behavior, Owner lifetime and
cleanup remain covered by the existing toolbar/action suites. No default
capability or public feature was removed.

## Measurement and verification

Release builds use `opt-level=z`, full LTO, one codegen unit and no debug info;
wasm-bindgen 0.2.129 is followed by Binaryen 132.0.0 `-Oz`. Rust/Cargo are
1.98.0 and Node is 24.3.0. Both matrices compile at the same frozen D-drive
source/target paths with the same lockfile. Ambient build flags are cleared;
Cargo metadata verifies the target directory. Each run archives source hashes
and resolved feature trees. See the runner README for exact commands.

Raw bytes and Brotli quality 11 are reported below. JS totals include the
wasm-bindgen entry and transitively imported snippets, compressed per resource;
the test loader is excluded. CSS files, fonts and images are separate assets.
Brotli is a transport estimate, not memory usage or runtime speed. Binaryen
also removes debug names, so the entire wasm-bindgen-to-Binaryen delta must
not be described as executable-code savings.

Both matrices passed all 36 browser cases in Chrome 155.0.8059.84: mount/unmount,
counter updates, CSS, messages, async completion, hydration node identity,
resource handoff, editor input and representative widget output. Glory's
render/HTML suites and Sapid's full `scripts/check.ps1` validate semantics and
strict Clippy independently. The used-widget cases exercise specific options;
they are not a size estimate for every configuration in a widget family.

`run.cjs --compare benchmarks/wasm-size/baseline.json --check` rejects increases
above 0.5%, with floors of 1 KiB raw WASM and 512 B Brotli/JS. Unused-feature
premiums have separate 1 KiB raw / 512 B Brotli growth limits when both members
of the reference pair are measured. Toolchain, profile, optimizer, compression,
lockfile and JS accounting must match. Intended growth requires an explicit
baseline review. `compare.cjs candidate.json baseline.json --check` can check
saved reports without rebuilding.

The negative control replayed the archived pre-optimization source at the same
path. It reproduced the original text WASM exactly (920,659 B, matching
SHA-256) and the gate rejected its 5,692-byte regression against the optimized
baseline. Replay refreshes restored source timestamps: Windows copies can
otherwise leave old timestamps that let Cargo reuse newer dependency artifacts.
The runner also tests that a shared runtime reduction cannot hide growth in an
unused-feature premium.

## Results

All values are bytes. “Before” and “After” use the same source paths and
compiler settings; after is the regression baseline. Feature premiums should
be read against the `reference` case in `cases.json`, rather than treating
different widgets as identical workloads.

<!-- wasm-size-results -->

| Case | WASM before | WASM after | Saved | Brotli before | Brotli after | JS after |
|---|---:|---:|---:|---:|---:|---:|
| empty | 1,379 | 1,379 | 0 | 803 | 803 | 4,175 |
| glory-minimal | 899,792 | 894,092 | 5,700 | 283,106 | 282,146 | 59,232 |
| glory-text | 920,659 | 914,967 | 5,692 | 288,913 | 287,806 | 59,232 |
| glory-button | 924,724 | 919,032 | 5,692 | 290,662 | 289,367 | 59,232 |
| glory-css-unused | 920,308 | 914,618 | 5,690 | 288,714 | 287,657 | 59,232 |
| glory-hydrate-unused | 921,203 | 915,597 | 5,606 | 289,378 | 288,134 | 59,232 |
| glory-resources-unused | 920,855 | 915,163 | 5,692 | 289,087 | 287,994 | 59,232 |
| glory-i18n-unused | 921,349 | 915,657 | 5,692 | 289,133 | 287,960 | 59,232 |
| glory-ssr-unused | 920,659 | 914,959 | 5,700 | 288,745 | 287,847 | 59,232 |
| glory-stream-unused | 921,415 | 915,801 | 5,614 | 289,245 | 288,155 | 59,232 |
| glory-all-unused | 922,545 | 916,933 | 5,612 | 289,315 | 288,360 | 59,232 |
| glory-css-used | 992,800 | 987,102 | 5,698 | 319,038 | 317,931 | 59,232 |
| glory-hydrate-used | 989,046 | 976,467 | 12,579 | 317,127 | 314,401 | 60,758 |
| glory-resources-used | 988,485 | 982,783 | 5,702 | 310,617 | 309,573 | 59,232 |
| glory-i18n-used | 1,044,865 | 1,039,175 | 5,690 | 334,703 | 333,672 | 59,232 |
| glory-ssr-used-in-browser | 948,301 | 942,606 | 5,695 | 295,607 | 294,983 | 59,232 |
| glory-stream-handoff-used | 1,066,168 | 1,053,585 | 12,583 | 342,903 | 340,094 | 60,852 |
| glory-stream-script-embedded | 968,117 | 962,503 | 5,614 | 300,159 | 299,021 | 59,232 |
| sapid-unused | 920,659 | 914,967 | 5,692 | 288,913 | 287,806 | 59,232 |
| sapid-button | 1,029,679 | 995,151 | 34,528 | 319,564 | 310,071 | 59,236 |
| sapid-charts-unused | 920,659 | 914,967 | 5,692 | 288,913 | 287,806 | 59,232 |
| sapid-chat-unused | 920,659 | 914,967 | 5,692 | 288,913 | 287,806 | 59,232 |
| sapid-code-unused | 920,659 | 914,967 | 5,692 | 288,913 | 287,806 | 59,232 |
| sapid-files-unused | 920,659 | 914,967 | 5,692 | 288,913 | 287,806 | 59,232 |
| sapid-editor-unused | 920,659 | 914,967 | 5,692 | 288,913 | 287,806 | 59,232 |
| sapid-json-unused | 920,659 | 914,967 | 5,692 | 288,913 | 287,806 | 59,232 |
| sapid-dates-unused | 920,659 | 914,967 | 5,692 | 288,913 | 287,806 | 59,232 |
| sapid-all-unused | 920,659 | 914,967 | 5,692 | 288,913 | 287,806 | 59,232 |
| sapid-button-all-unused | 1,029,864 | 995,411 | 34,453 | 319,408 | 310,379 | 59,236 |
| sapid-charts-used | 1,251,561 | 1,219,020 | 32,541 | 371,578 | 364,126 | 59,236 |
| sapid-chat-used | 1,031,451 | 997,271 | 34,180 | 321,002 | 311,474 | 59,236 |
| sapid-code-used | 1,083,942 | 1,078,252 | 5,690 | 334,127 | 333,327 | 59,236 |
| sapid-files-used | 1,008,236 | 1,002,536 | 5,700 | 311,136 | 309,816 | 59,236 |
| sapid-editor-used | 1,115,270 | 1,109,714 | 5,556 | 342,004 | 340,803 | 59,236 |
| sapid-json-used | 1,438,324 | 1,409,341 | 28,983 | 417,362 | 409,770 | 59,236 |
| sapid-dates-used | 1,297,026 | 1,268,521 | 28,505 | 380,905 | 373,592 | 59,236 |
