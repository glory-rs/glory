# Inert regions and pointer targeting

`Semantics::inert` applies only to View. It preserves content, layout, identities,
owners, resources and names while suppressing interaction and accessibility
exposure through the current semantic ancestry, including boxless Groups.
It is independent of hidden, disabled, busy and modal metadata. It is not a
general scroll lock: queries, layout/resource observations and programmatic
scrolling continue. Programmatic value/style updates remain possible.

Memory uses the same ancestor rule for enabled/focus checks and event routing.
User activation, editing, keyboard navigation, menu/choice/range commands and
pointer entry/movement cannot publish callbacks from an inert origin. This also
guards events that were already queued before an inert patch was accepted.
Terminal focus/composition loss, pointer leave and drag cancellation still clean
up live Owners; disposed identities remain invalid. The UI checks routing again
after an earlier callback changes the semantic tree, so a stale derived
FocusWithin/PointerWithin callback cannot reopen the blocked region.

HTML emits real `inert` plus an internal declaration marker. Web layer handling
keeps only current layer declarations and computes effective inert from semantic
ancestors. It forces the attribute onto the layer itself and excludes blocked
layers from input/focus management. Automatic modal background restoration
preserves explicit semantic inert. Classic streaming keeps a current layer-key
set, recomputes inherited inert after accepted edits/moves, and drops that set
when its controller ends. Hydration permits an extra empty layer `inert` only
when its supplied semantic ancestry independently requires it; validation does
not repair the document. There is no visited-layer history or application
collection scan.

Native focus and Tab traversal use the same semantic ancestry. The shell blocks
engine input defaults before a blocked control can acquire a caret or change
selection. Wheel/programmatic scrolling remains independent. AccessKit omits
the complete inert branch and retires its identities; reactivation allocates
new AX identities while retaining the underlying semantic and engine nodes.
Late actions against retired AX identities are ignored.

`Style::pointer_events` is a separate finite physical-targeting declaration:
`PointerEvents::{Auto,None}`. None passes pointer hits through the box and leaves
focus, keyboard and AX semantics unchanged. The property has independent profile
rejection; both values reuse existing Auto/None value bits, keeping the u64
support masks bounded. HTML/RECORD profiles are version 9; Blitz is version 10.
CSS parsing accepts only auto/none and checks every original declaration before
installation. Native hit tests prove None, Auto and absence/default reset on the
same engine identity. Author CSS does not expand this finite typed vocabulary.

Focused evidence: render/UI `tests/inert.rs`, HTML's full-shape differential
oracle (including clear/reset), style `tests/pointer_events.rs` and full-feature
sheet property coverage, native `accessibility_inert_tests.rs` and
`shell_inert_tests.rs`, native `tests/pointer_events.rs`, and SSR stream
`busy_inert_content_and_retained_layer_survive_a_stream_patch_and_end`.
`D:/BuildCache/sapid/inert-protocol.log` records the full render/UI/HTML/SSR
regression; `inert-native.log` and `pointer-native.log` record focused real native
behavior. Sapid loading-overlay-browser.json records nine actual Chromium
148.0.7778.96 scenarios against unified Web12, including Chrome AX exclusion,
retained open top-layer blocking, modal restoration and classic stream End followed
by real WASM adoption with stable element identity.
