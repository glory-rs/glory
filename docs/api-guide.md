# Glory API Guide

The active API uses semantic builders from `glory::prelude`, an application
`App`, and Owner-scoped `Cage`, `Bond` and `Vary` handles. The previous HTML
builder API, context container and holder lifecycle have been removed.

Start with the [cross-platform counter](../examples/counter/README.md) and its
[shared component](../examples/counter/src/lib.rs). The same component mounts
through Web or Apace; host-specific startup stays in the outer entry points.
A small headless example is [semantic_counter.rs](../crates/glory/examples/semantic_counter.rs).

## State and ownership

Construct state through a live Scope or Owner. Reads inside a Binding, Bond or
Effect track that immediate consumer. A bare component-build read does not
subscribe the component. Writes enqueue work; runtime flushes and host turns
apply it. Owner disposal releases observers, callbacks and tasks.

Pass application context explicitly through widget fields or captures. A copied
reactive handle keeps the lifetime of its original Owner; copying it does not
transfer ownership. Large collections should use indexed `with` reads and the
windowed list API instead of cloning or scanning the entire source on scroll.

For one selected key shared by many rows, `scope.selector(selected)` builds an
Owner-owned index over a `Cage<Option<K>>`. Construct each row's
`selector.selected(key)` under that row's Owner and read the returned
`Selected<K>` through `computed(move || selected.get())`. Direct
`Selected::get` reads use the current source even before a flush; only old/new
key channels normally publish notifications on a flush. A bounded set of live
channels actually read ahead of that notification also publishes once, including
after a batch returns to its original key, to invalidate cached intermediate
results. A cached `Bond` or a `Binding`
depending on that handle may retain its previous result until the flush. Flush
before observing those composed results; this finite notification contract does
not provide the same immediate propagation as ordinary Cage/Bond dependencies.
Each live key/Owner adds one notification source and cleanup. This optional
tradeoff reduces selection broadcasts; it does not make arbitrary predicates
indexed or remove the cost of mounting a full nonvirtual list.

Portable Link targets accept relative URLs and http/https/mailto/tel. Encode
spaces in URLs; executable and custom schemes are rejected before any host
mutation. Applications handle custom navigation through explicit callbacks.

## Rendering hosts

- [Web and SSR fixture](../examples/semantic-web/): semantic DOM, snapshot
  adoption, resources and progressive HTML acceptance scenarios. Build with
  `scripts/build-semantic-web.ps1`.
- [Salvo adapter](../crates/salvo-adapter/README.md): synchronous semantic SSR and
  optional pull-driven streaming responses.
- [Apace adapter](../crates/apace/README.md): native document, CPU paint and
  optional desktop shell, assets and AccessKit.
- [Localization](../crates/i18n/README.md): opt-in immutable Fluent catalogs and
  explicit application-local language selection.
- [Routing](../crates/routing/README.md): opt-in portable routes, retained layouts,
  history and SSR bootstrap. Use `link_with` with an explicit core Scope for
  asynchronous guards; `link` is the synchronous helper. Task execution requires
  a Host supplied by the application, without an implicit executor.

See the [semantic contract](semantic-ui-contract.md) for supported properties,
events, queries, styles and adapter limits, and the [implementation ledger](semantic-ui-implementation.md)
for verified behavior and remaining work. Legacy routing, server-function
and CLI sources were removed; the new routing crate is separately opt-in. The new
[project-local CLI](../tools/cli/README.md) provides separate host tooling,
locked by each application through its own small workspace executable.
