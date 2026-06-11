# Devtools Protocol

Glory exposes a small serializable inspector protocol from `glory_core::devtools`.
It is intentionally read-only: inspector code can observe scheduler and renderer
state without mutating runtime internals.

## Snapshot Model

- `ReactiveSnapshot`: one `Cage` or `Bond`, including id, version, subscriber
  count, subscriber view ids, and `dependency_ids` for reactive graph edges.
- `CommandQueueSnapshot`: command-buffer counts, event-handler counts, pending
  query counts, next node/query ids, and coalescing state.
- `DevtoolsSnapshot`: protocol version plus a batch of reactive and command
  queue snapshots.

`Bond::devtools_snapshot()` includes the ids of currently tracked dependencies.
`Cage::devtools_snapshot()` reports an empty dependency list because it is a
source node.

## Wire Messages

`DevtoolsMessage` serializes as a tagged JSON enum:

```json
{"type":"hello","payload":{"protocol_version":1}}
```

Supported messages are:

- `hello`
- `snapshot`
- `reactive_snapshot`
- `command_queue_snapshot`
- `command_batch`
- `warning`

Use `DevtoolsMessage::to_json()` and `DevtoolsMessage::from_json()` for the
canonical encoding. The `command_batch` payload uses the same
`renderer::Command` wire format consumed by desktop/native/liveview backends.

## Static Panel

`render_snapshot_panel(&DevtoolsSnapshot)` returns self-contained HTML for a
basic inspector view. Hosts can serve that as a debug page or embed it in a
desktop devtools window while a live streaming UI is built on top of
`DevtoolsMessage`.
