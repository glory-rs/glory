# Application input observations

`KeyboardInput` lowers physical keys to the finite `KeyboardCommand` set. Repeat,
composition and Alt combinations remain native editing. Editable controls receive
Submit only with a registered local interest; unhandled input is never prevented.
App routes commands through logical Groups to the nearest live handler, preserving
the physical origin. Native and browser listeners enqueue before UI execution.

Drag interests belong to semantic Views. Hosts exclude interactive controls,
start after a six-pixel threshold and hit-test the release point. Drop can target
a live sibling; other phases target the source. Embedded foreign, removed and
nonfinite origins fail shared validation. Moves coalesce only at the event tail.
Cancel and source retirement remove host state. A following click is suppressed.

`ColorSchemeChanged` observes the host preference, separately from an explicit
application theme. Browser media-query and native winit changes enqueue this
observation. New native interests adopt the current preference. Listener teardown
belongs to its application/document; no business callback runs inside a native
listener.

Builders can publish `node_ref` into a Cage. Retirement clears only the matching
identity, preserving a replacement branch's reference. `Scope::downgrade` allows
event/query helpers to upgrade transiently without retaining an owning UI Scope.
Named Dialog Views map to HTML and native accessibility dialog roles; applications
still own dismissal, focus and modality.

Validation evidence for this branch lives under
`D:/BuildCache/inkson-glory`: `framework-clean-input-tests1.txt` (complete render/UI
suite), `framework-clean-native-tests1.txt` (three native keyboard, pointer and
appearance probes), `framework-clean-browser1.txt` (two real Edge DOM probes,
including multiple applications and twenty remounts). These probes do not assert
actual OS IME acceptance, screen-reader announcements or authenticated app flows.

## Editor dismissal regression (2026-10-24)

Web and the native WindowApp driver allow Escape from a focused editor to reach
its nearest live Keyboard observer. The editor need not register a no-op handler.
Enter is different: Submit still requires that editor's own Keyboard interest;
ordinary multiline Enter and Shift+Enter retain their editing behavior. Existing
composition, repeat, Alt and prevented-event filtering stays in force.

`native_dismiss_bubbles_from_editor_without_stealing_submit_or_ime` drives actual
WindowApp key events, checks deferred delivery and original source identity, and
fails when only the routing condition is reverted. The shell library suite ran
43 tests successfully (one pre-existing ignored test). The runnable browser
fixture exports `keyboard_mount`; `keyboard-acceptance.js` ran against freshly
built WASM in real Edge and checked multiline editing, ancestor dismissal,
nearest-observer ownership, explicit Submit, composition filtering and teardown.
Composition events in that browser test are a protocol probe, not OS IME evidence.
