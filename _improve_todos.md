# Glory current maturity gap tasks vs local Dioxus checkout

> Scope: current Glory repo at `D:\Works\glory-rs\glory` compared with local
> Dioxus checkout at `E:\Repos\dioxus` on 2026-06-11.
>
> This is a fresh task board. `_todos.md` is still useful as a historical
> implementation log, but it contains older unchecked entries that are later
> marked complete in the same file. Treat this file as the current maturity
> board.

## Status Legend

- `[ ]` Not started.
- `[~]` In progress / partially implemented.
- `[x]` Done in the current codebase or completed while creating this board.

Priority:
- `P0`: blocks a credible mature release.
- `P1`: high user value, near-term.
- `P2`: medium-term product maturity.
- `P3`: long-term or optional differentiation.

## Current Verified Baseline

- [x] **A0-1** Local repo inventory completed. Glory currently has 13 workspace
  crates; Dioxus has roughly 45 packages plus first-party harnesses and
  examples.
- [x] **A0-2** Core architecture rechecked. Glory now has `CommandNode`,
  command-stream rendering, SSR replay through command DOM, desktop wry host,
  server functions, mobile templates, `asset!`, and a Blitz DOM consumer spike.
- [x] **A0-3** Remaining gap identified as mostly productization rather than the
  original renderer abstraction problem: CLI bundle matrix, CI/e2e coverage,
  docs/tutorials, fullstack batteries, devtools, and native/mobile completion.
- [x] **A0-4** Parallel work lanes defined below so independent work can be
  assigned or run concurrently.

## Parallel Lanes

These lanes can mostly move in parallel because the command-stream foundation is
already in place.

```text
Lane A: release governance + docs
  independent, unblocks contributors and release confidence

Lane B: CLI, bundle, templates, platform serve
  depends on existing desktop/mobile/runtime code; independent of fullstack

Lane C: fullstack and server capabilities
  depends on serverfn/runtime; independent of native/mobile packaging

Lane D: platform backends
  D1 desktop polish, D2 mobile runtime/CI, D3 native Blitz/vello, D4 LiveView
  share command protocol but can be separate branches

Lane E: performance and size
  benchmark/trace first, then optimize only measured bottlenecks

Lane F: testing and conformance
  should run in parallel with every feature lane

Lane G: API polish and ecosystem ergonomics
  typed forms/events/document/stores; mostly additive but needs docs/examples
```

## Maturity Comparison Snapshot

| Area | Dioxus local checkout | Glory current state | Gap |
|---|---|---|---|
| Core rendering | VDOM + mutation stream, many consumers | Fine-grained widgets + command stream | Mostly closed architecturally; needs more conformance and docs |
| Web CSR/SSR | First-class web, SSR, hydration, tests | CSR + SSR + command replay | Needs broader hydration and browser e2e matrix |
| Desktop | Mature wry/tao platform, APIs, tray/menu/window examples | wry runtime, multi-window/menu, custom protocol | Needs packaging, docs, API examples |
| Mobile | `dx serve --platform android/ios`, templates, docs | Android/iOS compile paths and templates | Needs emulator/device run, CI, bundle/install flow |
| Native | Experimental Blitz/wgpu stack | Blitz DOM consumer spike | Needs window/paint/event loop |
| LiveView | First-party crate | Not implemented; command protocol can support it | New crate + transport + lifecycle |
| Fullstack | Server functions, streaming, SSE, middleware, forms, uploads, server state | Basic server functions + adapters + context | Needs batteries and examples |
| CLI | create/serve/build/bundle/doctor/config, harnesses, platform matrix | build/serve/bundle/check/test/e2e/new/doctor; desktop/native bundle collection now exists | Needs templates, config schema, deeper mobile packaging |
| Assets | manganis, CSS modules, optimizer, resolver | `asset!`, asset mirroring, desktop protocol | Needs typed asset manifest, CSS modules, image optimization |
| Testing | Playwright suite, CLI harnesses, CI main workflow | targeted Rust tests, benchmarks, e2e hook | Needs CI matrix + Playwright projects |
| Devtools | devtools packages/types | subscriber debug helpers only | Needs inspector protocol/UI |
| Docs/community | extensive docs site/tutorials/examples | README + examples index + planning docs | Needs docs site and API guide |

## Lane A - Release Governance, Docs, Task Hygiene

- [x] **A1 P0** Create this current board with explicit parallel lanes and
  verified baseline.
- [x] **A2 P0** Reconcile stale historical entries in `_todos.md`: earlier
  unchecked rows for SSR convergence, desktop hot reload, server context,
  desktop protocol, multi-window, and benchmark evaluation conflict with later
  completed notes. Backfilled the rows with completion notes and kept truly
  pending items (mobile device/CI, bundle, Blitz next stage) open.
- [x] **A3 P0** Update `docs/multi-platform-rendering-plan.md`; it still says
  Stage 0/1 are unfinished, but the code now has command-stream SSR, desktop,
  mobile templates, and Blitz consumer. Rewritten as current status plus next
  stage plan.
- [x] **A4 P1** Refresh README target wording so it no longer presents Glory as
  only browser + SSR.
- [x] **A5 P1** Refresh the CLI internal README command list to include the
  actual command surface (`bundle`, `check`, `fmt`, `new`).
- [x] **A6 P1** Add a release-readiness checklist covering feature matrix,
  required targeted tests, examples to smoke, docs.rs feature set, and packaging
  checks. Added `docs/release-readiness.md`.
- [x] **A7 P2** Add `CODEOWNERS`, issue templates, and PR template analogous to
  Dioxus' contributor workflow.

Parallelism: A2 and A3 are independent docs cleanup; A6/A7 can run in parallel
with every feature lane.

## Lane B - CLI, Bundle, Templates, Platform Serve

- [x] **B1 P0** Implement non-web `glory bundle` targets. Current
  `crates/cli/src/command/bundle.rs` explicitly bails for non-web targets.
  Split into `bundle_web`, hosted binary bundle, Android artifact collection,
  and iOS artifact collection.
- [x] **B2 P0** Add desktop bundle output for Windows/macOS/Linux: executable,
  assets root, config metadata, icon path, and launch smoke docs.
- [x] **B3 P1** Add Android packaging beyond library compile: Gradle wrapper
  orchestration, APK output path, `install`/`run` convenience, emulator/device
  selection. `glory bundle --target android` now discovers the generated
  Gradle host, runs `gradlew`/`gradle`, copies APKs to
  `dist/<project>/android/apk/`, emits PowerShell/sh install/run scripts, and
  supports `GLORY_ANDROID_DEVICE`, `GLORY_ANDROID_INSTALL`,
  `GLORY_ANDROID_RUN`, and `GLORY_ANDROID_GRADLE_TASK`. True emulator/device CI
  remains in D2/F7.
- [x] **B4 P1** Add iOS packaging beyond staticlib compile: XcodeGen invocation,
  simulator/device target selection, `.app`/archive output. `glory bundle
  --target ios` now runs XcodeGen and xcodebuild on macOS, copies `.app`
  bundles and optional `.xcarchive` outputs under `dist/<project>/ios/`, writes
  simulator install/run scripts, and supports `GLORY_IOS_SCHEME`,
  `GLORY_IOS_SDK`, `GLORY_IOS_DESTINATION`, and `GLORY_IOS_ARCHIVE`.
- [x] **B5 P1** Add `glory doctor` for toolchain checks: Rust targets,
  wasm-bindgen, Binaryen/wasm-opt, cargo-ndk, Android NDK, Xcode/XcodeGen,
  webview runtime hints. The implementation checks Rust/cargo tools,
  target-specific rustup targets, wasm-bindgen/wasm-opt for web,
  Android NDK/cargo-ndk, iOS host/Xcode/XcodeGen, and desktop/native runtime
  notes.
- [x] **B6 P1** Add built-in templates instead of relying only on
  `cargo-generate` remote templates: web, SSR, fullstack, desktop, mobile.
  `glory new --template <web|ssr|fullstack|desktop|mobile> --name <app>`
  now writes local templates directly; explicit `--git` / `--path` still
  forwards to cargo-generate for external templates.
- [x] **B7 P2** Add a JSON/TOML config schema and validation command comparable
  to Dioxus' CLI config packages. Added `glory config`, `glory config --json`,
  and `glory config --schema`.
- [x] **B8 P2** Add asset optimization to the CLI: hashing, gzip/brotli
  precompression, cache manifest, optional image conversion/minification.
  `glory bundle` now precompresses static text/wasm assets and writes a
  seahash/size cache manifest in `glory-bundle.json`; image conversion remains
  intentionally non-default.
- [x] **B9 P2** Add CSS modules or scoped-style support if it can stay compatible
  with the builder API. Added `web::scoped_css` / `ScopedStyle`, selector
  prefixing for regular selectors and common nested at-rules, SSR coverage, and
  docs in `docs/styling.md`. Full build-time CSS Modules remain out of scope.

Parallelism: B1/B2 can start immediately; B3 and B4 can run in parallel after B1
establishes bundle target structure; B5 is independent and should start early.

## Lane C - Fullstack Batteries

- [x] **C1 P0** Add fullstack example parity beyond Hacker News: TodoMVC
  fullstack with SSR first render, server functions for mutations, and hydrated
  client updates. Added `examples/todomvc-fullstack` with `list/add/toggle/
  clear` server functions, SSR/CSR entry points, and verified both `web-ssr`
  and wasm `web-csr` checks.
- [x] **C2 P1** Add redirect and HTTP error helpers for server functions so
  handlers can return typed status/headers instead of only JSON success/error.
  Added `ServerFnHttpError`, `ServerFnError::http`, `ServerFnError::redirect`,
  adapter status/header propagation, and typed round-trip tests.
- [x] **C3 P1** Add form-action helpers around server functions: decode form
  posts, validation errors, optimistic UI example. Single-struct server
  functions now decode `application/x-www-form-urlencoded` posts, and
  validation helpers return typed HTTP 422 errors. Optimistic UI remains an
  example/docs follow-up under C1/G5.
- [x] **C4 P1** Add streaming response support: server functions or resources
  returning chunks/SSE, with SSR/hydration rules documented. Added
  adapter-agnostic `StreamingResponse`, `SseEvent`, and NDJSON helpers with
  tests and docs. Salvo/Axum/Actix adapters now expose streaming response
  glue for custom routes. Added `NdjsonDecoder<T>` and `SseDecoder` for
  hydration-aware client-side chunk consumption after SSR preloaded state.
- [x] **C5 P1** Add file upload and multipart support with size limits and
  adapter coverage for Salvo/Axum/Actix. Added `MultipartLimits`,
  `MultipartForm`, multipart parser, request-context detection, and tests.
  Added direct Salvo/Axum/Actix upload route recipes in
  `docs/serverfn-adapter-recipes.md`.
- [x] **C6 P2** Add WebSocket/SSE examples and helper APIs. Added SSE frame
  helpers, streaming response API, adapter streaming glue, client-side
  `SseDecoder`, framework-neutral `WebSocketFrame` and `TransportMessage<T>`
  helpers, typed JSON encode/decode tests, and SSE/WebSocket route recipes.
- [x] **C7 P2** Add server state/cache helpers, including invalidation and SSR
  preloaded state integration. Added process-local `ServerState<T>`,
  `ServerCache<K, V>` with TTL and explicit invalidation, and `PreloadedState`
  JSON/script-tag helpers for SSR handoff, with tests and docs in
  `docs/fullstack.md`.
- [x] **C8 P2** Add auth/session example using request context and cookies.
  Added `RequestContext::cookie()`, `set_cookie_header`,
  `clear_cookie_header`, redirect header chaining, tests, TodoMVC session
  display, and login/logout cookie redirect recipes in
  `docs/serverfn-adapter-recipes.md`.

Parallelism: C2/C3/C5 are independent API tracks; C1 should integrate the first
usable subset. C4/C6 share streaming transport decisions.

## Lane D - Platform Backends

- [x] **D1 P0** Desktop product polish: document `DesktopConfig`, multi-window,
  menu, custom protocol, hot reload, and server function remote URL in a
  dedicated guide. See `docs/desktop.md`.
- [~] **D2 P0** Mobile emulator/device validation. Added
  `scripts/mobile-device-smoke.ps1` and `docs/mobile-validation.md`; the script
  runs the mobile counter host check and can install/launch Android APKs or iOS
  `.app` bundles. Local run produced
  `target/mobile-device-smoke/mobile-device-smoke.json` with host check
  completed and `adb` auto-discovered under `ANDROID_HOME/platform-tools`, but
  real device validation is externally blocked here because no Android device
  or emulator is online and this machine is not macOS.
- [x] **D3 P1** Mobile lifecycle handling: safe-area defaults, keyboard resize
  policy, background/foreground scheduling, reload reconnect behavior. Mobile
  bootstrap now uses `viewport-fit=cover`, safe-area CSS variables,
  visual-viewport keyboard inset variables, `glory:viewport`,
  `glory:foreground`, and `glory:background` custom events; Android templates
  set `windowSoftInputMode="adjustResize"`.
- [x] **D4 P1** Native Blitz next stage: use `blitz-shell`/vello to show a real
  window from `BlitzConsumer`, then wire click/input events back into
  `EventData`. Added `glory-native/shell` with a `launch_blitz_window`
  entrypoint, a `GloryBlitzDocument` wrapper, reverse Blitz/Glory node mapping,
  and click/input event bridging back through `CommandHolder`.
- [x] **D5 P1** LiveView crate: command stream over WebSocket, server-held
  `CommandHolder`, browser interpreter, reconnect semantics, route/adapters.
  Added `glory-liveview` protocol/session primitives, reconnecting browser
  client, desktop interpreter event/query hooks, and a first-party Salvo
  WebSocket route that keeps the non-`Send` widget tree on a dedicated session
  thread.
- [x] **D6 P2** TUI demonstration graduation: decide whether to keep it as
  read-only command DOM visualization or build an interactive ratatui backend.
  Decision: keep `glory-tui` as a read-only command DOM visualization/debug
  aid, not a product renderer; crate docs and outline-rendering tests reflect
  that boundary.
- [x] **D7 P2** Platform API examples: file dialogs, window focus/size/position,
  clipboard, eval/script bridge, native menu shortcuts. Added
  `docs/platform-apis.md` covering desktop window config, multi-window, menus
  as host callbacks, file-dialog integration points without forcing a
  dependency into core, command-backend node queries, mobile viewport/lifecycle
  events, and `EventData` payload families.

Parallelism: D1, D2, D4, and D5 can be separate branches. D5 should reuse the
desktop interpreter and command protocol tests.

## Lane E - Performance, Size, Architecture Hardening

- [x] **E1 P0** Run official `js-framework-benchmark` Chrome tracing workflow
  against Glory and Dioxus-style baselines, not only the local harness. Added
  `benchmarks/official-js-framework-benchmark.ps1`, which clones/reuses the
  official repo, generates `frameworks/keyed/glory-rs` and
  `frameworks/keyed/dioxus-rs`, and drives the official `npm run bench --
  --framework ...` path. Ran the official Chrome/Puppeteer tracing CPU suite
  for `01_` through `09_` against both adapters with `-Count 1 -Headless
  -NoThrottling`; JSON results and the built result viewer are under
  `target/benchmark-report/official-js-framework/`. The run also caught and
  fixed Glory CSR composite-view `Each` reordering and benchmark remove-icon
  clickability issues.
- [x] **E2 P0** Add automated benchmark reporting for `Each`, command wire,
  SSR render, hydration, and CSR update workloads. Added
  `benchmarks/report.ps1` to run Criterion command-wire, Each, scheduler, and
  SSR stream benches plus wasm size JSON into `target/benchmark-report/`.
  The report now also records Playwright CSR/hydration/fullstack project
  status/logs and has a manual CI workflow that uploads the report artifact.
- [x] **E3 P1** Measure wasm size for `_test-size` and counter under debug,
  release, `wasm-opt -Oz`, and with/without routing/serverfn. Raw cargo wasm
  baseline and repeatable PowerShell harness are in
  `docs/performance.md` / `benchmarks/wasm-size.ps1`: `_test-size` 1.24 MiB
  release, counter 1.29 MiB release, router 1.49 MiB release,
  fullstack/server-fn 1.61 MiB release. The JSON report records skipped cases
  and local `wasm-bindgen`/`wasm-opt` availability; final bindgen/Oz/download
  rows are conditional on those optional tools being installed.
- [x] **E4 P1** Profile scheduler hotspots: subscriber lookup, `IndexMap`
  ordering, command queue allocation, and batch flush behavior under large
  reactive graphs. Added and ran `crates/core/benches/scheduler.rs` for
  shared-signal subscriber patching and command queue drain/coalescing, wired
  into `benchmarks/report.ps1`, and recorded local results in
  `docs/performance.md`. Coalescing remains opt-in because measured drain cost
  is effectively free while coalescing adds microsecond-scale work.
- [x] **E5 P1** Add memory/leak diagnostics for stale `Cage` handles, owner drop,
  event listener detach, and command node removal. Added per-type recycled-slot
  diagnostics for `Cage`; existing owner-drop, event detach, command DOM stale
  removal, and desktop detached-node release paths are covered by tests/code.
- [x] **E6 P2** Re-evaluate JSON command protocol only after measurements show
  IPC serialization over 10% of frame time or LiveView network payloads are too
  large. Existing `crates/core/benches/command_wire.rs` and `_todos.md`
  baseline conclude JSON remains acceptable; re-open only if profiling crosses
  that threshold or LiveView payloads make network size the bottleneck.
- [x] **E7 P2** Add streaming SSR performance tests and backpressure checks.
  Added `crates/core/benches/ssr_stream.rs` comparing `render_string()` and
  collected `render_stream()` chunks, with local results in
  `docs/performance.md`. SSR app output now chunks at DOM boundaries instead
  of one giant app subtree chunk, with stream poll-order coverage.
- [x] **E8 P0** Add a CSR bulk DOM/template creation path for large keyed lists.
  Official `js-framework-benchmark` tracing against Glory/Dioxus/Leptos shows
  Glory's bulk-create gap is dominated by script time, especially
  `07_create10k` (`~617 ms` total, `~366 ms` script in the latest local
  `Count=1` run) versus Dioxus (`~291 ms`) and Leptos (`~404 ms`). Small
  lifecycle and attr/class micro-optimizations were not enough. Prototype a
  renderer-level path that can build repeated static element skeletons through
  `template.content.cloneNode(true)` or `DocumentFragment`, then bind dynamic
  text/classes/events after clone. This is the main route to reducing
  wasm-to-DOM call count for `run1k`, `replace1k`, `create10k`, and append.
  Status: first validation prototype kept one Glory `View` per row but rendered
  the row's static DOM directly through CSR `Scope` node-placement helpers.
  Latest `Count=1` official run improved Glory aggregate `823.5 ms -> 558.4 ms`
  and `07_create10k` script `366.5 ms -> 167.7 ms`; next step is extracting
  this from the benchmark into a reusable template/bulk API. Follow-up
  template-clone prototype plus bulk `Each` detach improved the same 9-case
  aggregate to `437.8 ms` versus Dioxus `446.0 ms` and Leptos `563.5 ms`;
  `07_create10k` is now `313.5 ms` total / `83.8 ms` script. Landed the
  reusable CSR `DomSubtree` / `dom_subtree` widget in `glory-core`, migrated
  the benchmark row off its private `Widget` lifecycle implementation, and
  re-hid the raw `Scope` DOM-placement helpers as crate-internal API. Latest
  `Count=1` official rerun after extraction: Glory `442.4 ms`, Dioxus
  `455.9 ms`, Leptos `589.1 ms`; `07_create10k` Glory `305.6 ms` total /
  `78.3 ms` script versus Dioxus `296.9 ms` total / `64.5 ms` script.
- [~] **E9 P0** Compress per-view allocation on CSR hot paths. Each benchmark
  row currently creates a `RowWidget` view plus nested element views/scopes,
  `ViewId` path strings, owner state, maps, and boxed widget/attr/class values.
  Explore an internal "element child arena" or compact `Scope` mode for
  element-only subtrees so repeated rows do not allocate a full independent
  scope for every static `<td>/<span>`. Keep the public builder API; break
  internal layout compatibility if measurements justify it. Status:
  `DomSubtree` collapses benchmark rows from a `RowWidget + tr/td/a/span`
  view tree to one row view with native DOM children, validating this as a
  high-impact architecture branch. Remaining work is automatic/internal
  compaction for ordinary builder element subtrees. `Each` also gained a bulk
  detach path to remove many children without repeated `IndexMap::shift_remove`,
  improving `replace1k` `42.4 ms -> 32.0 ms` and `clear1k_x8` `8.3 ms -> 5.6 ms`
  in the latest exploratory official run.
- [ ] **E10 P1** Add stable multi-sample benchmark comparison output. The
  exploratory runs here used official Chrome tracing with `-Count 1`, which is
  enough to identify bottleneck classes but too noisy for accepting/rejecting
  small optimizations. Extend `benchmarks/official-js-framework-benchmark.ps1`
  reporting to preserve named baselines, run Glory-only A/B quickly, and emit
  median/min/max tables for `Count >= 5`.
- [ ] **E11 P1** Investigate event-handler installation cost separately from
  DOM creation. Glory already delegates bubbling events globally, but each row
  still creates and stores per-node wasm closures for select/remove. Build a
  synthetic benchmark with identical DOM but no row handlers, then one with
  delegated row id lookup, to quantify how much of `create10k` is closure
  setup versus node/attr/text insertion.

Parallelism: E8, E9, and E11 can proceed independently after E10 establishes
stable A/B reporting. E8 is the highest-impact path for Dioxus/Leptos parity;
E9 is a deeper internal architecture branch; E11 isolates whether event setup
needs a separate delegated-row API.

## Lane F - Testing, CI, Conformance

- [x] **F1 P0** Add a main CI workflow. Current `.github/workflows` has format
  and release only; Dioxus has a full main workflow. Added
  `.github/workflows/main.yml`.
- [x] **F2 P0** CI matrix: `cargo fmt --all --check`, `cargo clippy`, targeted
  `glory-core` default/web-ssr/backend-command tests, serverfn tests, CLI tests.
  The workflow also runs conservative feature checks for `glory-core`,
  `glory`, and `glory-serverfn` adapter features.
- [x] **F3 P1** Add Playwright test projects for web CSR, SSR hydration, routing,
  fullstack serverfn, and hot reload. Added first-party URL-driven Playwright
  projects under `tests/playwright` for CSR counter, CSR routing, SSR
  hydration, fullstack server functions, and hot reload script injection.
  `ssr-simple-salvo` and `todomvc-fullstack` now point their `glory end2end`
  metadata at these projects. CI runs install/list/skip smoke; full browser
  server provisioning remains a later CI hardening step.
- [x] **F4 P1** Add CLI harness crates for minimal web/SSR/desktop/mobile
  manifests, similar to Dioxus' `packages/cli-harnesses`. Added
  `glory-cli-harnesses` with local fixture manifests and CI coverage for
  Cargo metadata parsing.
- [x] **F5 P1** Add feature-combination compile tests for mutually exclusive
  `web-csr`, `web-ssr`, `backend-command`, `single-app`, and adapter features.
  Added source-level `web-csr`/`web-ssr` exclusion, CI expected-fail checks for
  invalid feature sets, and explicit docs.rs feature metadata.
- [x] **F6 P2** Add cross-backend conformance snapshots for more widgets:
  forms, SVG/math, head/meta, route changes, resources, error paths. Added
  SSR and command-stream snapshots for form controls and SVG/MathML, and made
  common `web::widgets::svg` / `web::widgets::math` builders public and
  compiled. Server-function success/error response parts are now shared by the
  Salvo/Axum/Actix adapters and covered for 200, 303 redirect + cookie, 404,
  and 422 validation parity.
- [x] **F7 P2** Add mobile CI smoke where runners permit it: Android compile
  always, emulator smoke optional/nightly, iOS compile on macOS runner. Added a
  `mobile-smoke` job to `.github/workflows/main.yml` that checks
  `examples/mobile-counter`, runs the CLI mobile-template regression test, and
  validates Android/iOS host template entrypoints. Emulator/device execution is
  tracked separately in D2.

Parallelism: F1/F2 can land first; F3/F4/F5 are independent once CI exists.

## Lane G - Public API and Ecosystem Ergonomics

- [x] **G1 P0** Audit typed HTML/event coverage against Dioxus `packages/html`.
  Fill missing form/input/pointer/keyboard/media/visibility events with typed
  `EventData` conversions. Added `docs/html-event-coverage-audit.md`. DOM tag
  coverage now matches the local Dioxus checkout by real tag name (Glory has
  `html`/`portal` extra), SVG/MathML builders are namespace-aware on CSR, and
  standard event gaps (`doubleclick`, `dragexit`, `encrypted`,
  `interruptbegin`, `interruptend`, `loadend`, `timeout`) are exposed.
  Clipboard events now use `ClipboardEvent`, bubbling metadata was corrected
  for focus/mouse/pointer enter/leave, `mounted` auto-fires on CSR and
  command-stream builds, and `visible` uses CSR `IntersectionObserver` with the
  same host-dispatchable event name available to command/native/LiveView.
- [x] **G2 P1** Add document/head APIs: title/meta/link/script/style helpers,
  SSR output rules, CSR updates, and desktop/native behavior. Existing
  `head_mixin`, `html_meta`, `body_meta`, and generated head-tag builders now
  have an SSR snapshot covering title/meta/link output into `<head>`.
- [x] **G3 P1** Add mounted/query ergonomics around `NodeQuery`: bounding rect,
  value, scroll offset, focus, selection. Added `CommandNode::value`,
  `CommandNode::bounding_rect`, `CommandNode::scroll_offset`, typed result
  structs, and command-query tests. Focus/selection remain host-specific future
  query kinds.
- [x] **G4 P1** Add collection/store helpers for nested state updates, inspired
  by Dioxus stores but kept compatible with `Cage`/`Bond`/`Lotus`. Added
  `VecStoreExt` for `Cage<Vec<T>>` (`push_item`, `update_item_by`,
  `remove_items_by`, `clear_items`) plus `CageLens`, root `StoreExt::lens`,
  lens mapping to `Bond`, and Vec/Option/HashMap/BTreeMap helpers for both root
  cages and nested lenses.
- [x] **G5 P1** Add form controls and validation examples: checkbox/radio/select,
  file input, disabled/read-only, controlled/uncontrolled patterns. Added SSR
  conformance coverage for label/input/checkbox/select/option/textarea/button
  and a runnable `examples/forms-showcase` CSR app covering controlled and
  uncontrolled inputs, checkbox, radio, select, file input, readonly state, and
  derived summary output.
- [x] **G6 P2** Add devtools protocol and UI: reactive graph, subscriber counts,
  command batches, event dispatch, SSR/hydration diagnostics. Added
  serializable diagnostic snapshots for `Cage`, `Bond`, and `CommandQueue`
  (subscriber counts/views, versions, dependency ids, buffered
  command/handler/query counts), `DevtoolsMessage` JSON wire messages,
  command-batch transport, static HTML inspector panel rendering, and docs.
- [x] **G7 P2** Add docs website or generated API guide with runnable examples.
  Added `docs/README.md` and `docs/api-guide.md`, covering builder widgets,
  reactivity, lists/branches, forms, routing, SSR/hydration, server functions,
  styling, command backends, CLI commands, and runnable verification commands.
- [x] **G8 P3** Consider third-party renderer guide once LiveView/native are real
  enough to prove the extension boundary. Added
  `docs/renderer-author-guide.md` covering command consumption, event dispatch,
  query answers, synthetic lifecycle semantics, batching, and conformance
  checks.

Parallelism: G1/G2/G3 can move independently; G6 should wait for F3/F6 so it has
stable scenarios to inspect.

## Immediate Concurrent Work Batch

These were selected because they are independent, low risk, and improve the
accuracy of the project while deeper implementation tracks continue.

- [x] **I1** Generate current `_improve_todos.md`.
- [x] **I2** Update README target matrix to reflect current desktop/mobile/native
  state rather than the older browser+SSR-only story.
- [x] **I3** Update `crates/cli/src/readme.md` subcommand list to match the real
  CLI command surface.
- [x] **I4** Reconcile `_todos.md` stale unchecked entries with later completion
  notes.
- [x] **I5** Refresh `docs/multi-platform-rendering-plan.md` into a current
  status/next-stage plan.

## Suggested Execution Order

1. Run A2/A3/F1/F2 together: get docs and CI trustworthy.
2. Run B1/B2/D1/F4 together: desktop packaging and harness coverage.
3. Run C1/C2/C3/F3 together: fullstack user-facing maturity.
4. Run D2/D3/F7 together: mobile real-device maturity.
5. Run E1/E2/E3 before any deeper performance optimization.
6. Run D4 and D5 as separate exploratory branches sharing command-protocol tests.
