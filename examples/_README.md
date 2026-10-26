# Examples

Examples are standalone workspaces, excluded from the root Cargo workspace.
Checking the root workspace does not build them.

The current entry points are:

- [counter](counter/README.md): one semantic component shared by Web and Blitz
  desktop hosts, with controlled input, derived state and activation handlers.
  Its build script verifies that Cargo caches, targets and temporary files use D:.
- [semantic-web](semantic-web/): the runnable semantic DOM/SSR/hydration/stream
  fixture and browser acceptance scripts. Use `scripts/build-semantic-web.ps1`
  from the repository root to build it.
- [Salvo SSR example](../crates/salvo-adapter/examples/ssr.rs): synchronous and
  progressive HTML through the current HTTP adapter; see its README.

The old routing, HN, SSR-mode and Tailwind examples were removed with the former
Core context/holder runtime. Other retained examples still use retired APIs and
are extraction references, not supported tutorials. Do not use the old CLI or
`web-csr` / `web-ssr` feature names to build the current examples.
