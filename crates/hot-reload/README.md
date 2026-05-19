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
- 5 unit tests in `src/diff.rs` cover the AST diff machinery.

## What's missing

- **A macro to actually hot-reload.** Glory's design is intentionally
  builder-pattern, no `rsx!` macro. To hot-reload builder code we'd
  need either:
  - a `subsecond`-style runtime function patcher (rebuild + relink
    closures at runtime; see `_todos.md` §5 P2 and dioxus's
    `packages/subsecond`); or
  - an `rsx!`-equivalent thin macro purely for the parts that
    benefit most from hot reload (icons, static layout chunks),
    while keeping the main builder API unchanged.
- **Devtools wire format.** Today the hot-reload pipeline lives only
  inside the CLI process; nothing on the running app side listens to
  it apart from the JS patch payload being loaded.
- **`Cage` / `Bond` state preservation.** Without a stable identity
  for signals across reloads, any reload will wipe app state. The
  generational-box redesign in `_todos.md` §2 P0 is the prerequisite.

## Status

Crate is **kept** in the workspace (decision _todos.md §5 P2).
Removing it would require unwiring `HOT_RELOAD_JS` from the SSR
renderer and `ViewMacros` from `glory-cli watch`. The internals are
useful as a base for future hot-reload work; revisit after §2 P0
generational-box and the §5 P2 subsecond design land.
