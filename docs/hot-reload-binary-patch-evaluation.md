# Hot reload: binary function patching (subsecond) — evaluation

Status: **evaluated, deferred.** This is the design conclusion for `_todos.md`
lane HR (HR2). It records why Glory does not currently pursue a Dioxus
`subsecond`-style binary hot-patch, and what would have to be true to revisit.

## What Glory has today

Glory's hot reload is wired end-to-end for two tiers (see `_todos.md` lane HR
and `crates/hot-reload`, `crates/cli/src/service`):

- **Full reload** — the CLI rebuilds and the browser/webview reloads the page.
- **Style reload** — CSS link swap without a page reload.
- **Function reload** — `glory-cli` parses `reloadable_fn!` markers
  (`HotReloadFunctions`), diffs them, and pushes a `FunctionReloadBatch` over the
  reload websocket; `FunctionRegistry::replace` swaps the registered closure at
  runtime. State held in `Cage`s survives because only the closure body is
  replaced, not the surrounding tree.

The function-reload tier already covers the common "tweak event-handler / render
logic and see it live" loop **for closures that were registered through the
registry**. It does not cover edits to arbitrary functions, struct layouts, or
newly-added code paths.

## What `subsecond` does

Dioxus `subsecond` performs binary-level hot patching: the CLI compiles a thin
patch object for changed functions and the running process jumps to the patched
code via a per-function trampoline / pointer table, with TLS bookkeeping. It can
hot-swap whole function bodies (not just pre-registered closures) without a
restart, across crates.

## Why Glory defers

1. **Toolchain depth and platform risk.** `subsecond` depends on careful control
   of linking and per-platform trampoline mechanics (macOS / Linux / Windows
   each differ). That is a multi-month, `unsafe`-heavy subsystem; Glory's core
   currently carries `unsafe_code = "deny"` and the CLI is deliberately thin.
2. **The builder model already gives a clean registry seam.** Because Glory is
   builder-based (no `rsx!` macro), the natural hot-reload unit is a registered
   closure, which the existing `FunctionRegistry` swap handles safely in 100%
   safe Rust. Most "edit and see it" loops fit this seam; the marginal benefit
   of arbitrary-function patching is smaller than for a macro-DSL framework.
3. **State preservation has a cheaper path.** The remaining high-value gap —
   preserving component state across a reload — is better served by snapshotting
   `Cage`/`Scope` state and restoring it after a function-reload (tracked as HR1),
   which does not require binary patching.

## Revisit criteria

Reopen if **all** of these hold:

- The function-reload tier proves insufficient in practice (measured: developers
  routinely hit "had to do a full reload" because the edit was outside a
  registered closure), **and**
- HR1 (state-preserving function reload) has landed and is still not enough, **and**
- A maintained, cross-platform binary-patch crate exists that Glory can depend on
  rather than hand-rolling trampolines.

Until then, the function-reload + full-reload tiers are the supported story.
