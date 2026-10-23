# Finite disclosure semantics

`button().disclosure(value)` accepts `Lotus<Option<Disclosure>>`. A `Disclosure`
has exactly `expanded: bool` and `controls: NodeId`. `None` means that this
button is not a disclosure control. The target is a live `View` in the same
Runtime, including its generation; arbitrary strings, HTML IDs, and role
replacement are not part of this API. The target may be hidden with styles.

Use `view().node_ref(target)` and a Bond which maps `target.get()` to the
optional Disclosure. The real target must exist before a nonempty relation is
accepted. Bare invalid creation and patches fail before the UI queue or identity
allocation changes. Descriptor-local checks stay separate from Memory's live
cross-node checks.

`Patch::Disclosure(Change::{Set,Keep,Clear})` is a field transaction. Failed
validation, dropping the prepared guard, and unwinding restore the previous
field without touching stats, watermarks, or the reverse index. The index stores
only current target/referrer identities, removes empty buckets, and never keeps
visited nodes. Disposing a target clears the relation on its surviving buttons
as part of that disposal. Disposal visits the owned subtree and indexed
referrers, not unrelated nodes or application collections. A recycled target
slot cannot reconnect an old relation. Keep emits no physical edits. Changing
expanded does not rewrite a label, input value, selection, or composition.

HTML format is now **v5**, following v4's finite rich-document protocol. Style
profiles remain HTML/record v3 and Blitz v4. Views receive reserved automatic IDs;
buttons map to `aria-expanded` and `aria-controls`. IDs include the Runtime and
generation and are independent of stream preorder keys. Server and browser IDs
use separate domains. The browser reserves an additional non-recycled token on
the Document using `Symbol.for('glory.document.identity')`, so independently
initialized WASM modules cannot collide. The application must not assign these
reserved IDs itself. Server fragments composed from independent server processes
must not contain duplicate IDs: hydration rejects duplicate/external endpoints.

SSR snapshots and the independent streaming Renderer use the same validation
and identified shapes. Stream End may rekey data metadata while the actual
control ID remains stable. Hydration validates all attributes and the actual
server control-to-target relationship before any DOM repair. It then rebases IDs
to the new runtime without replacing controls or replaying input values. Failed
adoption leaves the original subtree unchanged. A controlled target which is
absent from the server snapshot is currently rejected during hydration; keep
closed drawers mounted and hide them using style, as in the application.

Blitz lowers the same HTML edits. AccessKit exposes expanded/collapsed state and
controls referring only to targets present in its live visible tree. Hidden
endpoints do not introduce dangling AX IDs. The existing Activate/Click path is
unchanged. This does **not** add or advertise Expand/Collapse actions, or claim
screen-reader announcement or UIA client acceptance for this new relation.

## Verification

- Render: wrong runtime/kind, stale generation, rejected creation, prepared
  rollback, Keep/Clear, retirement and slot reuse.
- UI: actual reactive Button builder and queue/allocation rejection.
- HTML: field-only lowering against the full-shape differential oracle,
  canonicalization and clearing surviving references before target removal.
- SSR: snapshots and independent stream preflight/real relation serialization.
- Blitz: native WindowApp → AccessKit consumer; real Activate changes expanded,
  controls points to the actual target, retirement removes state/relationship,
  Expand/Collapse are not advertised. The existing accessibility suite also runs.
- `scripts/test-disclosure-browser.cjs`: real Edge CSR, SSR, classic stream and
  hydration using `examples/semantic-web` compiled WASM; preserves typed draft,
  UTF-16 selection and element identity; multiple apps and an independent WASM
  module; real click, Clear/Restore, target retirement, and invalid relation
  rejection without DOM writes. Stream uses a nontrivial End key permutation.

Build the example with wasm-bindgen 0.2.129, run its `disclosure_fixture` binary
to the same output directory, serve that directory, and pass
`GLORY_TEST_BASE_URL`, `PLAYWRIGHT_MODULE`, and `GLORY_TEST_ARTIFACT_DIR` to the
browser script. All target directories, generated WASM, browser profiles, logs
and artifacts must remain on D:/BuildCache as required by AGENTS.md.
