# Third-Party Renderer Guide

Glory renderer integrations consume the command stream rather than linking into
widget internals. The stable boundary is:

- `glory_core::renderer::Command`
- `EventData`
- `NodeQuery`
- `QueryResponse`

Desktop, native, TUI, and LiveView all use this shape. New renderers should
start with the reference interpreter in
`crates/core/src/renderer/command_dom.rs` and the JavaScript webview
interpreter in `crates/desktop/src/wry_interpreter.js`.

## Node Table

Maintain a map from Glory node ids to host nodes. Id `0` is reserved for the
host root.

Implement these command families first:

- `Create`: allocate a host node.
- `Insert` and `Remove`: keep host order identical to the command order.
- `SetText`: replace all children with text-content semantics.
- `SetHtml`: only support it if the host has a trusted HTML parser; otherwise
  document it as unsupported for native-style renderers.
- `SetAttribute` / `RemoveAttribute`
- `SetProperty` / `RemoveProperty`
- `AddClass` / `RemoveClass`

Command consumers should ignore stale ids rather than panic. Stale commands can
happen when remote events race with removed nodes.

## Events

`AttachEvent { id, name, bubbles }` means the renderer should listen for a host
event and send back:

```rust
EventData::new(name, id)
```

Fill optional payload families when the host can provide them accurately:
`pointer`, `keyboard`, `target`, `clipboard`, `selection`, `scroll`, `resize`,
`media`, and `extra`.

`mounted` is synthetic and already auto-fires in CSR and command-stream holders.
Renderers do not need to echo it from `AttachEvent`.

`visible` is synthetic. Browser CSR uses `IntersectionObserver`; custom hosts
should dispatch `EventData::new("visible", id)` when their layout/visibility
engine reports that the node became visible.

## Queries

When the command stream emits:

```rust
Command::Query { id, token, kind }
```

answer with a `QueryResponse` carrying the same token. Return
`QueryError::NodeGone` for missing nodes and `QueryError::Unsupported` when the
host cannot answer that query kind.

Current query kinds:

- `Value`
- `BoundingRect`
- `ScrollOffset`

## Batching

Treat each drained command vector as one transaction. Apply every command in
order, then flush the host renderer once. IPC renderers should coalesce only at
transaction boundaries; Glory's command queue already has an optional coalescing
pass for redundant content writes.

## Conformance

Minimum checks for a renderer:

- replay `command_dom` scenarios for create/insert/remove/text/class/attribute
  behavior;
- run `cargo test -p glory-core --features backend-command --test command_backend`;
- verify event dispatch updates a reactive widget and produces a second patch
  batch;
- verify unknown node events are ignored;
- verify query responses wake the pending query and stale query ids are rejected.
