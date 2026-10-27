# Native TagsInput protocol

The Blitz shell admits `Semantics.tags_input` on an ordinary single-line Input
and connects the shared `TagSeparators` and revisioned `Event::Tags` protocol.
Sapid TagsInput/PillsInput retain their existing controller, atomic controlled
state, duplicate/size rules, removable rows and successful-value proxy. The host
does not maintain accepted tags, parse business lists or invent popup roles.

## Keys and composition

Configured Enter/comma/semicolon and empty Backspace use the shared key policy.
Ordinary caret keys, Space, Tab and unconfigured separators remain editor input.
Modifiers/repeats do not become tag commits; empty Enter retains form submission.
The exact focused, subscribed control must remain enabled, writable, active and
inside the current modal/focus scope. A policy patch preserves editor identity,
value, selection and revision.

AppKit empty deletion shares the existing single current command marker with
Multiple combobox. Its following physical key supplies modifiers. Deleting the
final character does not also remove a tag. AppKit paste is paired with its raw
key so clipboard text is read once; newline waits for the physical key's
tag-versus-form decision. Other input and focus retirement clear the marker.
It contains only a current NodeId and command kind, not clipboard text/history.

An uncomposed software IME commit containing one configured punctuation character
can commit the draft. A software newline becomes the same Enter path, including
empty form submission. Active composition and a terminal candidate following
empty Preedit remain normal IME text, even when that candidate is punctuation.
The native IME context is invalidated after consuming punctuation so an equal
accepted draft can replace the native text store. These are protocol tests;
actual device keyboard/IME acceptance remains separate.

## Clipboard provider boundary

For an admitted paste, the host reads one clipboard snapshot from the document's
ShellProvider. The provider's OS read/allocation precedes this API's returned
String; its memory is not represented as bounded UI payload storage. The adapter
checks the returned UTF-8 length before normalization, copying into an event or
editing, and checks the normalized length again. The clipboard body is limited
to 16 KiB and current canonical draft to 4096 bytes.

Delimited/newline paste sends one `TagsCommand::Paste`, containing the current
value, canonical pasted text and sorted UTF-16 selection. It leaves the editor
untouched until the component accepts its whole proposal. Plain paste edits the
same editor, then emits the existing text event; no second clipboard read occurs.
Oversized clipboard/current/result text produces a body-free rejection, retaining
accepted text and selection. Provider read failure preserves the selection too.

An explicit `Config::clipboard` now routes paste through the asynchronous
[native clipboard service](native-clipboard.md) and reuses this same bounded
parser/proposal path. Without it, an injected ShellProvider retains the prior
synchronous path, including its million-byte rejection/recovery evidence.
Desktop semantic CopyText and macOS system round trips are separately verified;
mobile system services and actual device input/reader acceptance remain open.

## Evidence

Native regression tests cover policy patches, ordinary input, AppKit pairing,
empty/final-character deletion, readonly/inert/hidden origins, UTF-16 selection,
atomic paste, budgets/recovery, software punctuation/Return and composition.
AccessKit retains a TextInput, value and SetValue action, without ComboBox/ListBox
roles. AT value replacement does not tokenize text; retired actions are ignored.

The isolated `/Volumes/Data/Caches/glory/native-tags-consumer` mounts actual Sapid
widgets in four themes. Five tests cover free values, duplicate/maximum rules,
removal, live selected proxy values, controlled refusal/caret/accepted observers,
UTF-16 and whole-batch paste refusal, Pills duplicate occurrences, IME/form Return,
same-tick input/commit, 256 tags below 1100 nodes, readonly/disabled and cleanup.
CPU snapshots verify actual varied control/text pixels. Existing menu and choice
consumers remain separate regression suites; Sapid runtime code is unchanged.

The mobile `tags-probe` displays three separator policies and command feedback.
It is a protocol fixture; it does not implement Sapid's tag controller. SDK build,
installation, launch, installed-binary hashes and initial frames are recorded in
`/Volumes/Data/Caches/glory/native-tags/result.json`. Real software-key/touch,
VoiceOver/TalkBack, OS clipboard and signed-device acceptance remain open.
