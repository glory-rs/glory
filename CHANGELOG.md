# Changelog

## Unreleased

The optional XML display frontend now includes framework-owned keyed data and
`Each`, conditional/content ranges, component children, `field!`, typed boolean
class bindings, generated component Props and a display prelude. Handwritten
and XML frontends share the sparse HTML runtime. The canonical rows benchmark
uses this API; an independent standard HTML-table fixture lives in the sibling
js-framework-benchmark checkout. See the display guide for supported browser
capabilities and ownership contracts.

Display moves snapshot selection once per consecutive move run, cutting queries
between dirty DOM mutations while retaining focus, selection and failure-prefix
behavior. The default benchmark remains XML; complete standard-table results
and reverted instance-record experiments are documented separately.

Glory uses the breaking semantic UI architecture: Owner-scoped Cage/Bond/Vary,
finite rendering contracts, handwritten builders, shared HTML mapping and
separate Web, SSR, Salvo and Blitz hosts. Optional resources, CSS and i18n keep
their documented dependency boundaries.

The previous HTML runtime, global scheduler/context APIs, routing package,
CLI, procedural macros, server functions, hot reload, command-wire adapters,
legacy examples and their CI/publishing entry points have been removed.
No compatibility feature aliases or old wire interpreter are provided.

New project-local tooling lives in the independent `tools/cli` library workspace.
Applications own a private runner, shared Cargo.lock and bin launchers for build,
run, watch/reload, diagnostics, checks and Web/server bundles. Tool and compiler
caches stay on D:, and Web builds select the resolved wasm-bindgen version.

Web Plans retain original physical parts, release partially retired members,
and batch typed local-field edits within individual patches. Identity validation,
controlled-input barriers and successful-prefix receipts remain in Rust.
The cross-mutation field submission experiment was reverted after paired
measurements showed regressions; its results remain documented separately.

See the [implementation ledger](docs/semantic-ui-implementation.md) for delivered
behavior, validation and remaining work. Earlier unreleased notes described
retired APIs and are available in Git history.

## 0.3.1

Historical baseline release; see Git history and the 2026-10-13 assessment.
