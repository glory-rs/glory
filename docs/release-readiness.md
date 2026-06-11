# Glory Release Readiness Checklist

Use this before tagging or publishing. Do not substitute `cargo test --workspace`
for the targeted checks below; some workspace-wide failures are historical and
should be fixed in dedicated PRs.

## Required Checks

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy -p glory-core --lib --features web-ssr -- -D warnings`
- [ ] `cargo clippy -p glory-serverfn --all-targets -- -D warnings`
- [ ] `cargo clippy -p glory-cli --lib --no-default-features -- -D warnings`
- [ ] `cargo test -p glory-core --lib`
- [ ] `cargo test -p glory-core --lib --features web-ssr`
- [ ] `cargo test -p glory-core --features backend-command`
- [ ] `cargo test -p glory-serverfn`
- [ ] `cargo test -p glory-cli --lib --no-default-features`

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
- [ ] docs.rs metadata uses an explicit compilable feature set, not `all-features`.
- [ ] `cargo package -p glory-core`
- [ ] `cargo package -p glory`
- [ ] `cargo package -p glory-cli`
- [ ] Release notes mention breaking removals.
