# Native choices and intrinsic Select

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

## Intrinsic Select

`Props::Select` is admitted as a real visible intrinsic control. The adapter
creates a caption and a private scroll panel from the shared immutable bounded
configuration (1024 options, 32 groups, 128 KiB). These physical rows are not
semantic aliases or a second application combobox controller. Keys remain
separate from labels and successful form values. Declaration references are
shared; selection patches do not clone labels, values or declarations.

Single controls show the selected label or placeholder and open one current
viewport popup. Multiple controls and single controls with rows > 1 show an
inline scrolling listbox. Groups have real label rows; disabled options/groups
cannot be newly selected. Multiple edits preserve program-selected disabled
values. The shared semantic contract continues to reject readonly Select.

Selected keys are a live property, not default HTML attributes. Every explicit
write, including an equal-value rejection, repairs physical state and acknowledges
the current revision. Keep performs no property write. Native proposals emit
ordered unique keys with that live revision; program writes emit no user event.
Physical option identities survive patches, ordinary moves and resume. Suspension,
disposal, disabling, outside pointer/focus and Tab retire the current popup lease.
There is no visited-option history or per-document option registry.

Arrow/Home/End/Page keys navigate available options; single collapsed navigation
commits, while an open popup keeps a separate cursor until Enter/Space. Escape
cancels that cursor and then falls through to a parent dialog when already closed.
Multiple controls support toggling, Shift ranges, Control/Meta cursor movement
and select-all. Unicode typeahead searches only the bounded current declaration,
with a 256-byte buffer and 700 ms reset. Pointer selection keeps host focus on
the Select; disabled clicks do not close it. The existing engine owns touch pan,
click suppression after pan, wheel and contained scrolling. A popup inside a
modal remains inside its semantic focus/input boundary even beyond the modal's
physical rectangle. Only the current popup repositions after viewport/layout
changes, including offscreen anchors.

AccessKit exports ComboBox + current ListBox for a collapsed control, or an
inline ListBox, grouped ListBoxOptions, selected/multiple/disabled state and
current active-descendant. Option actions validate the live owner, visibility,
modal/focus scope, enabled state and option availability. Collapsed/retired
option IDs cannot select a new control. Accessible values contain selected
labels, never business form values. The mobile platform accessibility bridge
and actual OS screen readers remain separate unfinished work.

## Evidence

The original failing consumer log remains at
`/Volumes/Data/Caches/glory/choice-consumer-tests.log`. All five actual Sapid
consumer tests now pass in four themes: Select, Combobox, MultiSelect,
Autocomplete and TreeSelect/Cascader. The isolated project remains at
`/Volumes/Data/Caches/glory/native-choice-consumer`; current results are in
`/Volumes/Data/Caches/glory/select-consumer-tests.log`. These tests mount the
complete widgets, dispatch choice commands, check selection/identity and draw
real CPU pixels; they do not stand in for actual OS input.

Native regressions cover keyboard/pointer selection, controlled rejection,
revision acknowledgement, option identity, disabled groups, multiple ranges,
modal Escape and pointer boundaries, suspension/resume/disposal, touch pan,
contained wheel, viewport changes, explicit Plan instances and accepted-prefix
watermarks. CPU frames at 1x/2x and Android/iOS `select-probe` initial display
are stored in `/Volumes/Data/Caches/glory/native-select`. The mobile probe
shows standalone single, rows and multiple controls, with Chinese and groups.
The full workspace including doctests passes 1015 tests; native/WASM strict
Clippy and headless no-default-feature checks pass. Ordinary Android/iOS
packages also build; the iOS device target links with deployment target 15.0.
The unchanged engine fork provenance is verified. Source/test/log/frame hashes
are recorded in `/Volumes/Data/Caches/glory/native-select/result.json`.
SDK installation/launch, package hashes and initial pixels are evidence only for
those operations; no injected input or OS-reader acceptance is claimed.

Menus/context menus/palette are connected in the subsequent
[native menu implementation](native-menus.md). TagsInput policy, rich editing,
file pickers, semantic clipboard and the other report items remain open.
