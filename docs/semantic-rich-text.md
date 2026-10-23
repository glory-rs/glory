# Finite rich documents

`rich_text()` is a semantic control. The value is `render::rich_text::Document`,
not Markdown, HTML, an engine object, or an arbitrary editable subtree. Application
code owns Markdown/storage codecs, undo history and formatting commands. The
builder and `RichValue` use the same monotonic revision rules as `Input`.

The finite tree contains paragraphs, headings 1–6, quotes, ordered/unordered/task
lists, code blocks, rules, tables with aligned equal-width rows, and cells. Inline
values are text plus bold/italic/strike/code/link marks, atomic images, or hard
breaks. A document has one root with ID 0 and distinct nonzero block identities.
A selection is an anchor/focus pair `(block ID, UTF-16 offset)`; images and hard
breaks occupy one selection unit. Surrogate interiors are rejected. Documents
require an editable leaf, at most 1 MiB of strings, 8,192 blocks, 32,768 spans,
32 levels and 64 table columns. Validation bounds the pending traversal stack
before adding children. Links and images have explicit safe-target policies;
no raw HTML, script/data URLs, event attributes or arbitrary style payloads exist.

`RichTextChanged`, `RichTextCommit` and `RichTextCommand` carry a complete finite
document, program revision and selection. Changes also report composition.
Commands are Undo, Redo, Bold, Italic, Strike and Code; the application performs
that document transaction. Old events are discarded and future versions fail
before a callback. `Action::RichSelection` moves a valid selection.
`Action::RichMarks` sets validated marks for subsequent collapsed-caret typing
without adding invisible content. `Action` is now Clone, not Copy. Moving the
caret invalidates that temporary override. `Query::RichSelection` is a normal
Owner-scoped asynchronous query; headless renderers reject interactive actions.
Read-only controls keep selection/copy but reject document edits and typing marks.

HTML and the classic streaming wire are version 4. Style profiles are unchanged:
HTML/record v3 and Blitz v4. SSR, stream and Web share finite document lowering;
the classic bundle and WASM module use the same DOM codec. IDs serialize as decimal
strings, including the full u64 range. A controlled equal-document echo preserves
DOM identity, caret and composition. A differing program value waits until
composition ends. Stream drafts survive server patches and hydrate through the
normal semantic event queue. Before WASM, paste/drop inserts bounded plain text
and task controls update only their finite task field. The default HTML/core graph
still has no serde; the finite JSON codec is HTML's optional `wire` feature.

Web uses actual contenteditable paragraphs/lists/tables, native selection, a
bounded finite DOM reader, plain-text paste and real browser IME. Native uses real
Blitz containers and actual Parley attributed editors in each leaf. Bold, italic,
link/strike and code backgrounds are drawn as attributed runs. Lists, table
alignment and task checkboxes are actual boxes/controls. Images are atomic Parley
inline boxes with real provider-loaded image nodes (160×120 CSS-pixel contain box),
not Markdown syntax or text placeholders. Cross-leaf selection, clipboard, caret
navigation, paragraph splitting/joining, empty-item list exit, explicit typing
marks and composition cancellation operate on the finite document. The active
selection pointer is identity-bound and cancelled on disposal/window focus loss.
Providers retain their normal explicit origin/resource budgets. Application blob
URLs and authenticated assets remain application responsibilities.

Engine provenance is in `engine-forks.json`: Parley retains upstream package
names/structure/license at `3e267c1f887cdf30d0efd2363a9ecffa265c95b7`, Blitz at
`695af0bc5e7f628fab42d153ab61a7399114cf80`. No managed Cargo checkout or vendored
copy is modified. Both Cargo lockfiles use these immutable Git sources.

Validation performed on Windows 2026-10-23:

- Render/UI/HTML/SSR all-feature tests, including touched-field differential patch
  lowering and rich stale/future revision, invalid UTF-16 and unsafe-mark rejection.
- Blitz all-feature tests, actual attributed layout/selection/composition tests,
  and `rich_render` actual PNG decoding/raster pixels for table/list/image output.
- `check-engine-forks.py --test`: clean genuine-fork checkouts; Taffy, Blitz DOM
  (including parallel construction), Blitz paint and Parley library tests.
- `scripts/test-richtext-browser.cjs`: real Edge keyboard input, atomic UTF-16
  selection, controlled identity retention, pending marks, CDP Chinese preedit and
  commit, table layout, task activation and unsafe-URL rejection.
- `scripts/test-richtext-stream.cjs`: real HTTP SSR/stream/WASM hydration, user draft
  preservation, live typed/Chinese input, paragraph splitting and Owner teardown;
  it also runs the existing full stream acceptance suite.

These are adapter/engine and real browser acceptance results. They do not establish
OS-native Chinese IME, screen-reader text-range announcements, every mobile keyboard,
or application encrypted image/network authorization acceptance. Native accessibility
exports semantic document text and read-only state; it does not advertise unsupported
text-range actions. Clipboard import intentionally accepts plain text, while finite
formatting belongs to the application's editor tools.
