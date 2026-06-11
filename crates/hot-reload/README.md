# glory-hot-reload

Utility types for dev-mode hot reloading. Glory's primary hot-reload
surface is builder-style relinking: application code marks stable
builder closures with `reloadable_fn!` or `reloadable_view!`, and
`glory-cli serve --hot-reload` emits replacement messages when those
marked closures move or change.

## What's in here today

- `FunctionRegistry` / `ReloadableFn` — a builder-style runtime
  relink primitive. Existing handles keep calling the latest closure
  body after `registry.replace(id, new_fn)`.
- `reloadable_fn!` / `reloadable_view!` — explicit stable markers for
  builder functions and view builders.
- `HotReloadFunctions` — scans those markers so `glory-cli serve
  --hot-reload` can emit function replacement websocket events.
- `patch.js` — the client-side reload runtime that dispatches
  `glory:function-reload` events. Embedded into SSR HTML via
  `glory_hot_reload::HOT_RELOAD_JS` (see
  `crates/core/src/web/utils/ssr.rs`).
- Unit tests cover function marker scanning, stable reload payloads, and
  runtime relinking.

## What's missing

- **Compiler transform.** There is no procedural macro that
  rewrites arbitrary builder-style closures into `reloadable_fn!`
  registrations automatically.
- **Native app-side relink transport.** The browser websocket now
  dispatches `glory:function-reload`; a desktop/native runtime still
  needs to consume the same event shape on its IPC channel.

## Removed legacy path

The old `view!` macro / virtual-node diff machinery has been deleted.
Glory's public component model is builder-pattern Rust, so retaining an
RSX-like template diff layer only created misleading data structures and
unused transport branches. Future hot reload work should extend the
function relink path instead of reintroducing template patch payloads.
