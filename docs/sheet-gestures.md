# Finite sheet gestures

`Layer.sheet: Option<SheetConfig>` refines a named managed-focus edge Dialog.
The configuration contains at most eight sorted finite fractions in (0,1],
current index, maximum logical extent (128–4096), settle milliseconds (0–500),
dismissible flag and non-wrapping revision. Other Layer kinds and non-fixed
explicit positioning are rejected. `Semantics.sheet_grip` marks a passive generic
View; it carries no invented range role or focus. A grip acts only inside the
nearest currently registered sheet in the same App. Native Layers/grips reject
in descriptor-local preflight and direct application, retaining accepted prefix
and query watermarks.

`Event::Sheet { from, revision, command }` targets the sheet itself. Its origin
must equal the live target. Snap indices and dismissal permission are checked
against the same revision; older events are discarded, future events rejected.
Both raw Descriptor patches and field-local prepared patches reject regressing
revisions and changed configuration/edge at an equal revision. A live sheet
declaration cannot be removed/re-added to reset its counter. Shared inert,
suspension, disposal and event routing rules remain in force.

HTML contains all finite config data, including the exact decimal revision.
The classic stream only applies this data and handles End; it does not become
an input driver. WASM adoption retains the real panel and grip. The browser
indexes current accepted declarations only, one captured gesture and one pending
release restoration. Raw native listeners record coordinates, capture identity
and terminal status; no layout is read there. RAF reads initial layout and the
visualViewport, then performs finite private geometry writes. Controlled refusal
restores accepted geometry after the whole host turn even when apply is never
called. Program configuration also writes a finite CSS expression without layout
reads so an explicit Bounds barrier sees its accepted dimension before next RAF.

Fixed intrinsic grip CSS reserves pointer capture for its hit area with
touch-action:none; content remains native scroll/selection/editor input. Snap
buttons are the accessible keyboard alternative. Four viewport edges and offsets
are supported. Window blur, capture cancellation, changed revision/viewport,
hidden/inert/suspended state and disposal revoke a stale gesture. Finite settling
transitions honor dynamic prefers-reduced-motion; no per-frame application task
or reactive writes are used.

Evidence: render `tests/sheet.rs`; HTML's full-shape field differential oracle
includes sheet config, equal-revision rejection and grip clear/reset; native
`tests/sheet.rs` proves descriptor/direct rejection and prefix query continuity;
SSR stream `bounded_sheet_config_and_revision_survive_stream_patch_and_end_without_input_driver`
exports `GLORY_SHEET_STREAM_FIXTURE`. Sapid component/browser documentation is in
its `docs/sheet.md`.

Sapid's `D:/BuildCache/sapid/sheet-browser.json` records nine actual Chromium
148.0.7778.96 scenarios against unified Web14, including captured mouse and touch
input on all four edges, zero-mutation refusal restoration, revision/viewport/blur/
inert cancellation, real settling geometry, live reduced motion and actual classic
stream-to-WASM adoption. `sheet-open-ax.json` contains four theme AX probes of
the named Dialog and accepted pressed snap button.
