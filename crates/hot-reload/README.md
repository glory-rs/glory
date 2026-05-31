# glory-hot-reload

Utility types for dev-mode hot reloading. This crate is **partial**:
it carries the foundation (AST parsing, virtual-DOM diff, the JS
patch payload) but Glory's runtime doesn't use the diff/patch loop
yet because Glory has no `rsx!`/`view!` macro to hot-reload against.

## What's in here today

- `ViewMacros` — walks Rust sources, finds `view!` /`rsx!` style macro
  invocations, snapshots their token streams keyed by file + span.
  Used by `glory-cli watch` (see `crates/cli/src/command/watch.rs`)
  to keep a per-project map of macro bodies for future reload work.
- `diff::Patches` and `node::LNode` — a lightweight virtual-node
  diff producing JSON patches against the JS-side renderer.
- `patch.js` — the corresponding client-side runtime that consumes
  the JSON patches. Embedded into SSR HTML via
  `glory_hot_reload::HOT_RELOAD_JS` (see
  `crates/core/src/web/utils/ssr.rs`).
- `FunctionRegistry` / `ReloadableFn` — a builder-style runtime
  relink primitive. Existing handles keep calling the latest closure
  body after `registry.replace(id, new_fn)`.
- `HotFunctions` — scans `reloadable_fn!("id", ...)` /
  `reloadable_view!("id", ...)` markers so `glory-cli watch
  --hot-reload` can emit function replacement websocket events beside
  macro patch payloads.
- 5 unit tests in `src/diff.rs` cover the AST diff machinery.

## What's missing

- **Compiler transform.** There is still no procedural macro that
  rewrites arbitrary builder-style closures into `reloadable_fn!`
  registrations automatically.
- **Native app-side relink transport.** The browser websocket now
  dispatches `glory:function-reload`; a desktop/native runtime still
  needs to consume the same event shape on its IPC channel.

## Status

Crate is **kept** in the workspace (decision _todos.md §5 P2).
Removing it would require unwiring `HOT_RELOAD_JS` from the SSR
renderer and `ViewMacros` from `glory-cli watch`. The internals are
useful as a base for future hot-reload work; see
`examples/builder_relink.rs` for the minimal builder-style pattern.
