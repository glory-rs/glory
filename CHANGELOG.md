# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Reactivity**: `use_future` / `use_future_in` for fire-and-forget async tasks
  scoped to a `Scope`, and `use_coroutine` / `use_coroutine_in` for long-lived
  message-driven coroutines with a typed channel handle (`Coroutine<M>`).
- **Reactivity**: app-wide reactive state via `GlobalCage<T>` / `GlobalBond<T>`
  and the `global_cage!` / `global_bond!` declaration macros — lazily initialized,
  shared across scopes, never owner-reclaimed.
- **Routing**: route ranking — sibling routes are now matched most-specific
  first (static > dynamic > catch-all) regardless of registration order, via a
  new `Filter::specificity` / `Router::specificity`. `ServerAviator::with_redirects`
  resolves declarative redirects during SSR. `#[derive(Routable)]` now auto-parses
  query parameters and transits typed params through `#[redirect]`.
- **Server functions**: incremental/ISR caching primitives — `RenderFreshness`,
  `IncrementalCache`, and a path-traversal-safe `FileSystemCache`.
- **Desktop**: `DesktopWindowHandle::eval(js)` returning a future of the JS
  result (`EvalError` on failure), bridged over IPC.
- **Document**: typed `<head>` helpers (`document::title`/`meta_name`/`stylesheet`/...)
  with dedup-on-key so duplicate declarations collapse.
- **Native (Blitz)**: form `value`/`checked` reflected into Blitz form state, and
  incremental layout-query resolution via a dirty-flag `LayoutCache`.
- **LiveView**: session registry with idle/TTL reaping and resume tokens
  (`LiveViewConfig`, `SessionRegistry`) so disconnected sessions can be resumed
  or garbage-collected; protocol-version negotiation on `Hello`.
- **Server functions**: HTTP-status error helpers (`ServerFnError::unauthorized`
  / `forbidden` / `not_found` / `conflict` / `bad_request` / `internal`) plus
  `http_status` / `is_client_error` / `is_server_error`, all round-tripping
  across the serialized boundary.
- **Desktop**: `DesktopCloseBehavior` (close vs hide on window-close) with
  `DesktopConfig::with_close_behavior`, and `DesktopWindowHandle::open_devtools`
  / `close_devtools` / `set_close_behavior`.
- **CLI**: tool version resolution now honours SemVer range requests in the
  per-tool version env var (e.g. `>=1.2, <2`), resolving to the latest matching
  release instead of treating the value as an exact pin.
- **Engineering**: workspace `CHANGELOG.md`; CI now runs doc tests, a SemVer
  check (`cargo-semver-checks`), MSRV verification, a `cargo publish --dry-run`
  gate, a benchmark compile gate, a cross-platform (Windows/macOS) check job, and
  `cargo-all-features` coverage.

### Changed

### Fixed

## [0.3.1]

Baseline release. See git history prior to this changelog for details.

[Unreleased]: https://github.com/glory-rs/glory/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/glory-rs/glory/releases/tag/v0.3.1
