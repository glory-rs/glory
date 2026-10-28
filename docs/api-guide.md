# Glory API Guide

The active API uses semantic builders from `glory::prelude`, an application
`App`, and Owner-scoped `Cage`, `Bond` and `Vary` handles. The previous HTML
builder API, context container and holder lifecycle have been removed.

Start with the [cross-platform counter](../examples/counter/README.md) and its
[shared component](../examples/counter/src/lib.rs). The same component mounts
through Web or Blitz; host-specific startup stays in the outer entry points.
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

## Rendering hosts

- [Web and SSR fixture](../examples/semantic-web/): semantic DOM, snapshot
  adoption, resources and progressive HTML acceptance scenarios. Build with
  `scripts/build-semantic-web.ps1`.
- [Salvo adapter](../crates/salvo-adapter/README.md): synchronous semantic SSR and
  optional pull-driven streaming responses.
- [Blitz adapter](../crates/blitz/README.md): native document, CPU paint and
  optional desktop shell, assets and AccessKit.
- [Localization](../crates/i18n/README.md): opt-in immutable Fluent catalogs and
  explicit application-local language selection.

See the [semantic contract](semantic-ui-contract.md) for supported properties,
events, queries, styles and adapter limits, and the [implementation ledger](semantic-ui-implementation.md)
for verified behavior and remaining work. The former routing, server-function
and CLI sources have been removed; they are not facade features. The new
[project-local CLI](../tools/cli/README.md) provides separate host tooling,
locked by each application through its own small workspace executable.
