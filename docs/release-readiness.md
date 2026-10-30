# Glory Release Readiness

This checklist applies to the active semantic workspace. The former Core
holder/command API, CLI, server functions and old adapters/examples have been
removed. The current [release workflow](../.github/workflows/release.yml) runs
the reusable CI gates before publishing the dependency-ordered
[`release.json`](../release.json) allowlist. See [RELEASING](../RELEASING.md).

The new [project-local CLI library](../tools/cli/README.md) is a separate tooling
workspace under `tools/cli`. It does not restore the removed CLI architecture or
enter the runtime graph. Cargo metadata defines the active package set,
including the optional display compiler. Counter owns its private launcher and
shares its application lockfile with that launcher.

## Required checks

- [ ] Run `./scripts/check-semantic-ui.ps1`: metadata, pinned engine sources and
  fork tests, formatting, all-feature workspace tests and strict Clippy. This
  gate also tests and lints the independent CLI workspace.
- [ ] Run its `-Web` checks and the current browser acceptance functions in
  `examples/semantic-web`; a WASM build alone is not browser acceptance.
- [ ] Run `-Audit` with cargo-audit 0.22.2 installed. Known vulnerabilities,
  unsoundness and yanked packages fail the gate. Unmaintained-package warnings
  remain visible in the JSON evidence and require maintenance follow-up.
- [ ] Verify the Rust 1.94 MSRV and portable Linux gates in `quality.yml`.
  The Apple job is opt-in and requires a prepared data-volume runner; it is
  separate from the Windows/Linux release checks.
- [ ] Verify the migrated counter with the commands in its README.
- [ ] Exercise Salvo synchronous and streaming responses, including body-driven
  backpressure and cancellation; see `crates/salvo-adapter/README.md`.
- [ ] For desktop claims, run the `-Window` checks and record the actual host,
  OS input and accessibility scope separately.

All build artifacts, Cargo caches and compiler temporary files must use
`D:/BuildCache/<task>` (or a verified D:-backed WSL mount). Verify Cargo's actual
`target_directory` with metadata before building.

## Publishing

- [ ] Review the current workspace package list with Cargo metadata; publish
  only supported packages in dependency order.
- [ ] Verify versions, lockfiles, package contents and docs.rs feature sets.
- [ ] If publishing `glory-cli`, verify its separate manifest, lockfile and
  library-only package, and test a private runner pinned to the same framework
  release/source. The release allowlist includes this independent workspace;
  root workspace package selection alone does not include it.
- [ ] Run package and publish dry-run checks for each intended package.
- [ ] Keep optional resources, i18n, CSS and native dependencies outside default
  graphs according to their documented boundaries.
- [ ] Record breaking removals and outstanding platform limitations in release
  notes. An all-feature workspace test does not prove retired packages work.

The [implementation ledger](semantic-ui-implementation.md) records remaining
architecture and platform work. This checklist does not authorize publishing.

Dependency evidence is produced by `scripts/check-dependencies.py` for the
runtime, CLI, maintained examples and Glory benchmark applications. Frozen peer
comparison fixtures are excluded from this release audit. Dependabot covers
the same maintained manifests. No advisory is silently ignored; unmaintained
transitive font and desktop-bus libraries need upstream migration tracking.
