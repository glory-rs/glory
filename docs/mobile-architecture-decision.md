# Mobile Crate Architecture Decision

Date: 2026-06-12

This note closes the M4 evaluation from `_todos.md`: whether Glory should move
desktop/mobile/web examples into one crate with heavy `cfg` feature gates, or
keep explicit mobile host templates.

## Current Shape

Glory mobile apps use:

- a Rust payload crate with `crate-type = ["cdylib", "staticlib", "rlib"]`;
- `mobile` as the selected feature for Android/iOS builds;
- generated Android and iOS host projects under the app template;
- `examples/mobile-counter` as the reference payload and host loop.

This keeps platform packaging files visible: Gradle, Android manifest, Kotlin
activity, XcodeGen YAML, and Swift entrypoint all live where platform developers
expect them.

## Decision

Keep the transparent template structure. Do not merge desktop and mobile into a
single cfg-heavy app crate right now.

The current approach has some duplication, but the duplicated files are exactly
the files users need to inspect when Android/iOS packaging fails. That matters
more than reducing template line count while mobile support is still before
real-device CI.

## Tradeoffs

Advantages of the current template:

- Android and iOS host configuration is explicit and debuggable.
- `cargo-ndk`, XcodeGen, package names, bundle IDs, and generated artifacts are
  easy to inspect without macro or build-script indirection.
- Web, desktop, and mobile features do not accidentally unify through a single
  workspace member.
- The command-stream backend stays opt-in for mobile payloads.

Costs:

- Shared app code may need to be copied or factored into a separate library by
  the user.
- Some host-loop code is duplicated between examples and generated templates.
- Multi-platform apps need a little more project structure up front.

## Revisit Triggers

Reopen this decision when:

- Android emulator CI and at least one macOS iOS simulator smoke are both green;
- at least two first-party examples share enough mobile/desktop host code to
  justify a reusable helper crate;
- users report that template duplication is a bigger problem than platform
  packaging visibility.

At that point, prefer a shared helper crate plus small platform host crates over
a single large crate that hides platform packaging behind nested `cfg` blocks.
