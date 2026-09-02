# Glory Release Readiness Checklist

Use this before tagging or publishing. `cargo test --workspace` is a useful
broad smoke check, but do not substitute it for the targeted checks below: the
feature matrix and command-backend paths catch failures a default workspace run
will not exercise. Feature-gated integration tests should declare
`required-features` in their crate manifest and be covered by an explicit
targeted command here.

## Required Checks

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy -p glory-core --lib --features web-ssr -- -D warnings`
- [ ] `cargo clippy -p glory-serverfn --all-targets -- -D warnings`
- [ ] `cargo clippy -p glory-cli --lib --no-default-features -- -D warnings`
- [ ] `cargo test -p glory-core --lib`
- [ ] `cargo test -p glory-core --lib --features web-ssr`
- [ ] `cargo test -p glory-core --features backend-command`
- [ ] `cargo test -p glory-serverfn`
- [ ] `cargo test -p glory-serverfn --features "salvo axum actix"`
- [ ] `cargo test -p glory --features routing`
- [ ] `cargo test -p glory-cli --lib --no-default-features`
- [ ] `cargo test --workspace`

## Feature Matrix

- [ ] `cargo check -p glory-core --no-default-features`
- [ ] `cargo check -p glory-core --features web-ssr`
- [ ] `cargo check -p glory-core --features backend-command`
- [ ] `cargo check -p glory-core --features "web-ssr backend-command"`
- [ ] `cargo check -p glory-core --features "web-csr web-ssr"` fails with the mutual-exclusion guard.
- [ ] `cargo check -p glory-core --features "backend-command single-app"` fails with the scheduler guard.
- [ ] `cargo check -p glory-core --features "web-csr backend-command"` fails with the scheduler/browser backend guard.
- [ ] `cargo check -p glory --no-default-features`
- [ ] `cargo check -p glory --features "web-ssr backend-command routing server-fn"`
- [ ] `cargo check -p glory-serverfn --features "salvo axum actix reqwest-client"`

## Example Smoke

- [ ] `examples/counter` CSR build or serve.
- [ ] `examples/ssr-simple-salvo` SSR + hydrate smoke.
- [ ] `examples/hackernews-salvo` server function endpoint smoke.
- [ ] `examples/desktop-counter` desktop launch smoke on a machine with WebView runtime.
- [ ] `examples/mobile-counter` Android/iOS compile smoke on the appropriate host.
- [ ] Manual run of `.github/workflows/mobile-device-smoke.yml` succeeds on the
  Android API 34 x86_64 emulator.

## Release Artifacts

- [ ] `glory bundle --release --target web`
- [ ] `glory bundle --release --target desktop`
- [ ] Verify `dist/<project>/glory-bundle.json`.
- [ ] Verify assets load from the bundle root for desktop.

## Documentation

- [ ] README feature matrix matches the current code.
- [ ] `_todos.md` completed items are checked; `_improve_todos.md` remains the
  2026-06-11 baseline archive.
- [ ] `docs/multi-platform-rendering-plan.md` does not describe completed work as future work.
- [ ] AGENTS.md mentions any changed feature constraints or wire protocol rules.

## Publishing

- [ ] Versions are bumped consistently across workspace crates.
- [ ] The next version is not already published on crates.io. If any package at
  the current workspace version is already published, bump the workspace version
  before cutting a new release.
- [ ] docs.rs metadata uses an explicit compilable feature set, not `all-features`.
- [ ] Publish and verify the core release crates in dependency order. Cargo
  rewrites path dependencies to registry dependencies in package tarballs, so a
  downstream crate may fail `cargo package` until its same-version internal
  dependencies are visible in the crates.io index:

  1. `cargo package -p glory-hot-reload`
  2. `cargo package -p glory-macros`
  3. `cargo package -p glory-core`
  4. `cargo package -p glory-routing`
  5. `cargo package -p glory-serverfn`
  6. `cargo package -p glory`
  7. `cargo package -p glory-cli`

- [ ] For each crate above, run `cargo publish -p <crate> --dry-run` before the
  real publish, then wait for the published version to appear in crates.io
  search before moving to the next downstream crate.
- [ ] If the release includes platform crates, run package and dry-run checks
  after `glory-core` is published for `glory-actix`, `glory-axum`,
  `glory-salvo`, `glory-desktop`, `glory-native`, `glory-liveview`, and
  `glory-tui`.
- [ ] Release notes mention breaking removals.
