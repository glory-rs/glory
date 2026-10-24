# Native viewport top layers

The genuine `glory-rs/blitz` fork is pinned to
`7e0b5f85727da016285028f656a437d89ca10c46`. The provenance record and Cargo
lockfiles resolve all four Blitz packages to that full commit. No vendored code,
managed Cargo checkout edits, global patches or DOM portals are involved.

`BaseDocument::set_top_layer` admits at most 1024 current leases. A lease keeps
the element's DOM/Stylo parent, selector matching and inherited style, while
excluding it from its parent's ordinary layout children. The engine lays it out
against the viewport, paints ordered backdrops and roots after ordinary content,
and hit tests them in reverse order. Ancestor clipping, transforms and opacity
do not trap the top layer. Hidden ancestors still suppress it. Connected moves
preserve a lease; detach/drop retires it before identity reuse. `contain_scroll`
is explicit and defaults to false; the adapter enables it for its managed layers
so wheel, touch, fling and editor scrolling use the existing engine scroll chain
without escaping to the document. No closed-layer history is retained.

Shared Memory admits at most 1024 live Layer facets, including hidden, detached
and suspended declarations. Its single numeric counter changes only on accepted
Create/Plan, Semantics guard commit or Dispose. Placement preflight and direct
apply check the same bound; Drop/panic rollback leaves the counter and watermark
unchanged. Thus native physical admission cannot fail after a mapper has already
accepted a 1025th typed layer. No per-node payload or history cache was added.

The Glory adapter currently supports named Dialog and AlertDialog layers with
Manage focus, plus nonmodal Dialog with Preserve focus. Center and all twelve
AnchorPlacement combinations resolve actual measured boxes. Missing, hidden,
inert or retired anchors suppress the physical layer and queue one current
AnchorLost dismissal; recovery permits the accepted controlled state to reopen.
Preserve does not autofocus, trap Tab or restore focus. Modal focus and AX omit
unrelated branches while admitting subsequent popups associated by a current
live anchor. Background capture is cancelled, late background AT actions are
ignored, and outside nonmodal dismissal preserves the user's clicked target.
Only active layers force geometry work; ordinary mutations and closed layer
declarations retain lazy layout.

Menu, Tooltip, Listbox and navigation Group layers, sheet, ContextPoint and a
Layer's own Translation remain explicit native rejections. Descriptor preflight,
placement preflight and direct apply retain their accepted-prefix and query
watermark contract. Supporting Dialog does not imply those facets or OS screen
reader acceptance.

Reproduce fork-owned tests with `python scripts/check-engine-forks.py --test`.
The final run is `D:/BuildCache/sapid/native-top-layer-final-provenance.log`:
Taffy 185, Blitz default 113, parallel/writing-mode 114, existing focused profile,
style, lifetime and image regressions, and one CPU paint test. The paint test
checks real pixels, ordered modal backdrops, huge ordinary z-index, ancestor
clip/transform/opacity escape, exactly-once alpha composition and close cleanup.
Focused runs overlap the full test suites and are not distinct-case totals.

Adapter tests are in `crates/blitz/src/layer_tests.rs` and
`accessibility_layer_tests.rs`. They exercise actual center/anchor bounds, all
twelve sides/alignments, focus cycling/return, controlled refusal, out-of-order
modal close, associated popup Tab navigation, Preserve behavior, real wheel
scroll containment and layout laziness. The AX consumer test verifies modal
scope, background action rejection and non-recycled mappings. Sapid's seven
actual AlertDialog/FloatingDialog/ColorInput consumer tests additionally cover
four-theme raster output, anchor/picker mutation and identity preservation:
`D:/BuildCache/sapid/native-new-layer-consumers-final.log`.

The final `glory-blitz --features shell,raster,accesskit --all-targets` run passes
195 tests across 45 result reports (including zero-test examples; library 93
passed and two ignored). Strict Clippy on the same targets passes with
`-D warnings`. Logs: `native-layers-final-all.log` and
`native-layer-final-clippy.log` under `D:/BuildCache/sapid`.

Web's separate Document coordinator is covered by the final root Web23 run:
67 suites, 633 reports, no errors, including twelve cross-App layer groups.
That browser evidence does not stand in for the native engine or driver tests.
