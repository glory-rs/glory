# Native choices: current connection and remaining intrinsic Select

The native adapter connects the shared `ComboboxKind::{Select,Editable,
Autocomplete,Multiple}`, `ChoiceCommand`, scalar Typeahead and Pointer protocol.
The component owns matching, an active cursor and controlled selection; the
host performs no business-source scan or alternate selection controller.

Choice commands require the current focused control and live interest. Editing,
composition, modifiers and readonly/disabled state keep their existing protection.
Multiple's empty Backspace emits RemoveLast; nonempty text uses the real editor.
On macOS, Winit delivers AppKit's command before the raw key. An originally
empty `deleteBackward:` cancels only its no-op deletion, retaining one current
NodeId until that following physical key supplies the actual modifiers. Other
input/focus retirement clears it. Deleting the final character does not also
remove a token; password Backspace is not replayed on the raw key. Regression
coverage here is for this pair; other AppKit command/raw-key pairs and actual
OS input still need their separate audit/acceptance. Collection search observes only an explicitly opted-in
boundary/navigation item and stops at the nearest boundary. The event contains
one printable nonspace scalar and the document's bounded monotonic clock;
components own query length, matching and timeout policy.

Listbox top layers Preserve focus. Pointer activation of the intrinsic option
Button never transfers focus away from the editor/trigger, including when the
controlled component closes the popup. Tab skips options. Tree top layers Manage
focus and restore the current trigger; they reuse the existing Tree navigation.
RevealOption resolves the closest matching live semantic listbox and reveals
only inside it. No surrounding document/viewport scroll is introduced.

Mouse/pen observations use the former/current hover targets within one input
dispatch, resolving engine generations to live semantic origins. Compatibility
mouse events do not duplicate the observations. App derives logical
PointerWithin boundaries across transparent Groups. Touch does not enter this
hover protocol. No hover/visited-option registry survives a dispatch.

AccessKit exports ComboBox/EditableComboBox, selected options, multiple listboxes
and an active-descendant relation only when current ownership and physical export
agree. Hidden, disabled, removed or replaced targets cannot remain active.
PasswordInput retains its protected role and no raw value, including under a
combobox refinement. Tree/action tests are separate from actual screen readers
and the still-missing mobile accessibility bridge.

## Intrinsic Select remains required

`Props::Select` is a different control from the combobox refinement on a Button
or Input. It remains rejected in preflight/direct apply. Actual Sapid Select,
Combobox and MultiSelect consumers create it as a hidden successful form proxy;
therefore these complete consumers still fail to mount. Preserve the failures in
`/Volumes/Data/Caches/glory/choice-consumer-tests.log` and the reproducible isolated
consumer project at `/Volumes/Data/Caches/glory/native-choice-consumer`.
Autocomplete and TreeSelect/Cascader pass four-theme command/raster checks; they
do not prove intrinsic Select or all consumers complete.

The next implementation must cover the actual control, including visible uses:

1. Bounded immutable declarations (1024 options, 32 groups, 128 KiB), stable
   option keys distinct from business values, disabled options/groups and empty
   selection/placeholder behavior. Reuse the existing shared validators.
2. Single-selection collapsed display/popup and multiple/rows listboxes with real
   CPU layout, fonts, scrolling, hit testing, keyboard and pointer interaction.
3. A live selected-key property separate from default attributes, ordered unique
   emitted keys and revision/prefix barriers. Program changes, controlled rejection
   and equal-value acknowledgements must preserve physical identities. Selection
   patches must not clone labels/values/declarations or scan application data.
4. Correct suspension/reattachment, disposal and popup cleanup, safe late input,
   readonly/disabled behavior and modal/focus-scope boundaries.
5. Real AccessKit option state/actions and current relations; optional/native
   work must stay outside default/HTML/SSR/WASM graphs. Retain OS acceptance bounds.
6. Turn all three currently failing actual consumers into passing tests, then
   verify standalone Select, mobile builds/display and the full framework gates.

Do not admit only hidden proxies or leave accepted selections invisible. Host
private physical children/option mappings may follow the bounded current control;
no business-source/visited-key history or new unsafe bridge is authorized.
