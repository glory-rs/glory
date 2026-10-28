# Finite rich editor protocol

The shared protocol and Web driver use HTML format 13. Native explicitly rejects
the facet until a native rich editing engine exists. Component acceptance and
the preserved first-browser failure are recorded in Sapid's `docs/rich-editor.md`.

## Value model

`RichDocument` is a finite ordered vector of paragraph/heading(1–6)/quote blocks.
Each block has ordered text runs with independent bold, italic, code and optional
http/https/mailto link marks. There is no raw HTML, style string, image, embedded
widget, script, template, or engine type. Paragraph breaks are block boundaries;
runs use canonical text without CR/LF/NUL. Adjacent equal marks merge, and empty
blocks have one unmarked empty run. Literal `<`, `&` and quotes stay ordinary text.

Limits are independent: 1–128 blocks, 1–2048 total runs, 64 KiB text, 2048 bytes
per link, 128 KiB aggregate link metadata. `RichSelection` has anchor/focus block
indices and UTF-16 offsets, including backward selection. Offsets must land on
scalar boundaries, never inside surrogate pairs. Current document text is small
and explicit; no business collection is enumerated or retained as visited history.

The pure model tests normalization, cross-block replacement,
exact UTF-16 ranges, selected-mark split/clear/merge, headings, safe link schemes,
and every budget. The separate Web driver supplies actual editing observations.

## Shared declarations and observations

`Props::RichEditor(RichEditorProps)` contains `Rc<RichDocument>`, monotonic revision
and enabled state; readonly uses the shared Semantics field. Its one main/control
node is a genuinely editable
multiline textbox. It has no semantic children: its finite document belongs to
the editor value, rather than arbitrary app-owned interactive children.

`Patch::RichDocument { document, revision }` uses a shared `Rc` payload (plus
ordinary Enabled and Semantics fields). Keep/Set/Clear, fresh lifetimes, revision equality,
regression and exhaustion follow Input rules; equal revisions require equal
canonical document. Live preparation swaps only the touched value and rolls
back on guard drop/unwind. The indirect payload preserves Patch's existing size
invariant. Descriptor-local preflight/direct/Plan/stream and the full-shape
field-difference oracle cover the new facet and existing variants.

`RichSnapshot` contains a finite document, UTF-16 selection, captured revision
and composing flag. `Event::RichEdit(Box<RichSnapshot>)` and a matching query
response carry actual projected host state, not raw markup. Old revisions drop,
future revisions reject, exact target/lifetime and payload validation precede
callbacks. Selection-only observation is separate from a document edit so moving
a caret never proposes a business value or adds an undo entry. Composition end
is terminal cleanup on live Owners, independent of stale document input.

`Query::RichSelection` returns a boxed actual snapshot: selection can belong to
a physical draft not yet acknowledged by a controlled source. The response
validates its own document/range and accepted request watermark. A controller
must compare the snapshot revision to its still-live node before applying a
format command; stale replies cannot edit a later source. Owner query retirement
uses the existing indexed cancellation path. `Action::SelectRich` carries an
explicit range and revision, validates it against the accepted value before
capability/host work, and does not silently focus another control.

Formatting, insertion and undo remain portable pure document operations. The
Sapid controller queries/observes the actual range, proposes a finite value,
then restores an accepted selection explicitly. It does not call deprecated
execCommand or inject an HTML fragment.

## HTML, SSR and classic stream

The pure mapper uses one stable main `div` with role textbox, aria-multiline and
contenteditable reflecting enabled/readonly. A finite typed RichDocument
property controls internal paragraph/heading/blockquote nodes and the bounded
`a/strong/em/code` run structure; the property is not an HTML string. Serializer
and host create those nodes from the same value, escaping text and attributes.
Document edits never replace the editable main/control identity.

The Shape carries its complete finite document to serialize/adopt/stream, with
an explicit EditKind. Full-shape delta tests must cover the new variant, default
resets, metadata/name/style/disabled changes, canonical echoes and property
barriers. All existing variants remain under their current oracle. Format 13
adds the typed property, finite document grammar/tags and independent budgets to
classic Start/Create/Patch/End; old 12 is refused without mutation.

Classic capture records only bounded dirty editor observations and composition
state. Hydration validates the entire subtree before consuming a dirty document
and selection, retains the actual control, and begins a fresh revision lifetime.
The classic renderer does not become an app input driver or keep edit history.

## Web host and composition

Listeners collect raw input/selection/composition notifications and prevent unsafe
rich paste/drop defaults. The host read phase projects only the current dirty
editor(s), before DOM writes, with finite node/text/depth budgets and a strict
element/mark grammar. Plain paste is text; no pasted markup is accepted. Cursor
projection uses actual DOM ranges and UTF-16 block offsets. Native browser
editing provides grapheme deletion; the model does not pretend scalar deletion
is a complete platform editor.

Native browser editing provides actual contenteditable typing and IME. Normal
document observations canonicalize the finite projection before queuing a typed
event. Composition uses one current draft. Canonical-equivalent source echoes
acknowledge the revision without rebuilding text or disturbing preedit. A
different program value during composition is one deferred desired value: keep
the draft's captured input revision until it ends, then apply the latest accepted
program value. Old late draft/commit observations never overwrite it. Cancellation
or Owner retirement removes current membership and late acceptance.

Selection reads and any caret repairs are a host observation/write operation,
not work from a DOM listener or component factory. Parse failure/budget overflow
has a typed result and restores only the last accepted finite value; it cannot
quietly swallow an unresolved live pending state. OS IME, screen-reader and
cross-browser acceptance must be stated separately from Chromium CDP input tests.

## Sapid component and bounded history

An optional dependency-free `editor` feature exposes RichTextEditor and its
controller/finite document types. It retains Cage/Bond/Vary, four themes,
unstyled and explicit style overrides, controlled source silence and internal/
typed-before-generic callback order. Toolbar buttons are outside the editable
control and use true toolbar/control semantics. No native fallback textarea is
created. Synchronous SSR emits a deterministic finite value without polling a
clock/task service.

Undo/redo stores at most 32 accepted snapshots and at most 2 MiB charged text,
link and finite block/run bookkeeping, evicting oldest entries. This is a logical
payload bound, not total RSS. A refused controlled proposal does not create a
history entry. External document replacement silently invalidates old history;
selection-only changes do not. An IME composition contributes one accepted edit,
not a snapshot per preedit update. New accepted edits retire redo. Pending
proposal/draft/history drops stay outside reactive/registry borrows.

## Acceptance scope

Pure model/property tests; shared validation, guard rollback, placement and
successful-prefix conformance; all mapper field oracles; SSR/classic/hydrate
dirty selection and version cases; native create/patch/query/action explicit
rejection before allocation; and actual Chromium four-theme CSR/SSR typing,
formatting, link safety, multiline editing, Unicode selection, composition,
controlled refusal/source replacement, bounded undo/redo, disabled/readonly,
teardown and mobile geometry are covered by the linked Sapid evidence. Immutable
Web43 passes original editor9, classic11 and extra3 on Chromium148; the earlier
Web41 exact-boundary failure remains recorded. Native rejects the rich facet
before allocation; this is not native rich editing or OS IME/AT acceptance.
