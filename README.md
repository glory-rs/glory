# Glory

Version 0.6.0 uses the published Apace, Takia and Wingy engine families.
Native applications use `glory::apace`, the `apace-*` features and
`glory-apace`. See [release instructions](RELEASING.md) and
[engine provenance](docs/engine-forks.md).

```toml
[dependencies]
glory = { version = "=0.6.0", default-features = false, features = ["dsl"] }
```

For production applications, enable symbol stripping in the application's own
workspace manifest. Cargo ignores release profiles declared by dependencies:

```toml
[profile.release]
strip = "symbols"
```

The benchmark workspaces use matching release profiles. See the
[strip and static-host analysis](docs/wasm-strip-static-host-2026-10-30.md) for
measured sizes, retained Widget/Each behavior and reproducibility details.

Glory is being rebuilt as a Rust semantic UI framework: handwritten builders,
owned fine-grained reactivity, and replaceable rendering hosts. No UI DSL or
virtual DOM is required.

This branch is an intentionally breaking implementation in progress. The
[architecture report](docs/semantic-ui-rendering-architecture-report-2026-10-21.md)
defines the target; the [implementation ledger](docs/semantic-ui-implementation.md)
records what actually works. The active Salvo integration is documented below;
the old HTML runtime, CLI, server functions and command adapters have been
removed. Current examples use the semantic API; historical reports remain in
Git history and the implementation ledger.

The new [project-local tools](tools/cli/README.md) are a library-only tooling
package. Applications own their executable and lock the CLI with their other
dependencies. The [counter](examples/counter/README.md) includes `bin/glory.ps1`
and `bin/glory` launchers for build, preview, watch/reload and diagnostics; no
global Glory CLI installation is required.

The migrated [counter example](examples/counter) shares one semantic component,
state, event handlers and typed styles between Web and Apace desktop hosts.
Its README includes build/run commands and the current platform boundaries.

The [mobile counter](examples/mobile/README.md) adds explicit Android NativeActivity
and iOS UIKit entries for that same component. Android has an isolated D: emulator
and APK/interaction lifecycle checks. Apple Silicon macOS has native window, AX
client and signed `.app` evidence; iOS 27 has simulator pixels, software keyboard,
pinyin IME, resume and rotation evidence. Signed device installation and Apple
production distribution remain pending. See the [Dioxus feature comparison](docs/dioxus-feature-gap-analysis-2026-10-26.md)
for the remaining platform and application gaps. SSR framework integration stays
limited to Salvo.

## Available now

- The opt-in [XML display API](docs/v2-display-keyed-content-guide-2026-10-28.md)
  extracts shared HTML templates and binds Cage/Bond/Vary fields. Raw HTML
  elements need no semantic Widget object. Handwritten configurations use the
  same runtime; `Each`, `Show`, content ranges, component children and optional
  `#[component]` cover ordinary browser composition. See the
  [counter/list example](examples/display-web/src/bin/counter.rs).
  Component tags now use the same `Widget` contract as Sapid and handwritten
  widgets: `<sapid::Button label="Save"/>` calls its ordinary builder.
  `#[component]` generates a Widget builder; there is no separate Component or
  IntoView trait. See the [unified Widget DSL guide](docs/v2-widget-dsl-unification-2026-10-29.md).
  Existing factories use `<{button(args)}/>`; mixed HTML/Widget trees work in
  synchronous SSR snapshots, browser launch/hydration and the native Widget host.
  Raw elements retain sparse template state. Incremental SSR shares template
  declarations, streams slot and Widget edits, then hands the final DOM to hydration.
  The legacy template AST/compiler and duplicate content controllers are retired;
  both frontends share checked declarations, binding actions and keyed/branch logic.
  [Unified Each](docs/v2-unified-each-2026-10-29.md) accepts vectors/Vary and
  KeyedList through the same builder and DSL key/children/seed contract.
- `glory-style`: explicit portable style values, separate from CSS engines.
  Ordered `Family` fallbacks, fractional font weights, `WhiteSpace`, border
  width/style/color, uniform `radius` and group `opacity` use the same mapping
  in HTML and Apace. Font selection and text layout remain host-owned.
  Optional `css` adds bounded `Sheet::parse` for the current typed properties,
  class/child/descendant selectors and five states. HTML lowering is shared by
  browser/SSR/Apace; each target checks its profile before installation. This is
  a finite subset, not complete CSS or a GTK cascade implementation.
- `glory-render`: eight semantic node kinds, typed properties/events/queries,
  generational node identities and a reference interpreter. Focus queries read
  actual control focus at acceptance; focus requests alone do not imply success.
- `glory-core`: owned Cage/Bond/Vary, direct dependency tracking, equality
  gates, bindings/effects, per-Runtime scheduling, logical scopes and tasks.
- `glory-resource`: optional request-scoped named resource registry and
  versioned JSON snapshots with explicit schemas and bounded input.
- [`glory-i18n`](crates/i18n): opt-in Fluent catalogs, explicit language sources
  and Owner-owned reactive messages. Enable facade feature `i18n`; core and
  default/HTML/SSR graphs remain independent of localization. Routing and SSR
  language negotiation belong to the application.
- `glory-ui`: App/Widget/Scope, view/group/text/button/input/check/image/link
  builders, each/when, and lazily indexed fixed or measured-height lists.
  Sparse Fenwick metadata stays bounded by the measurement cache and window.
  At most one focused row remains mounted outside the window until blur.
  Structural roles belong to `view`; a named generic View becomes an accessible
  group. Plain text uses content, Heading can have a name, and decorative images
  have no name/focus target. Metadata updates preserve editor values/selection.
- `glory`: a facade over those packages.
- `glory-html`: common HTML mapping, typed incremental edits and serialization.
  HTML format v3 includes semantic enabled/disabled/checked attributes on main
  mapping nodes; stylesheet states work for Check's internal control as well.
- `glory-web`: wasm32 DOM interpreter, RAF host, input/composition handling,
  real geometry/selection queries and windowed browser scrolling.
- `glory-ssr`: synchronous snapshots and document output using the same mapper;
  versioned fragments can be adopted with the optional `hydrate` feature.
- `glory-ssr/stream`: native pull-driven shell/edit/end frames with owned request
  cancellation and a CSP-compatible browser bootstrap before final hydration.
- [`glory-salvo`](crates/salvo-adapter): native Salvo responses for synchronous
  semantic SSR and optional genuine progressive HTML streaming. Its bounded
  local workers preserve body-driven backpressure and cancellation on disconnect.
  Run `cargo run -p glory-salvo --example ssr --features stream` for `/plain`,
  `/stream` and the matching browser script. Legacy Axum/Actix adapters are removed.
- `glory-apace`: native headless document adapter using the same HTML edits,
  actual CSS/box/text layout, viewport/DPI, measurements and programmatic scrolling.
  Enable optional `apace`; run `cargo run -p glory --example semantic_apace --features apace`.
  Optional `apace-paint` records frames and `apace-raster` produces CPU RGBA8
  pixels; unchanged frames skip raster work. Windows and Ubuntu/WSL have actual
  document and pixel-test evidence. Optional `apace-shell` adds a desktop CPU
  window/event-loop host with queued activation, focus, navigation, controlled
  editing and IME protocol.
  Password supports grapheme masking with matching pointer/caret/scroll geometry,
  raw controlled values and IME protocol. Web/SSR retain HTML password support.
  Config supports explicit resource
  providers with bounded engine caches and cancellation. Optional `apace-package`
  supplies an offline embedded-byte loader shared by desktop and mobile;
  `apace-assets` adds desktop HTTP(S)/mounted-file transport with explicit origins,
  directory roots and transport
  budgets. Shared request/completion leases keep decoded results bounded while
  the host is paused. Measured lists preserve anchors and navigation visibility
  through delayed intrinsic PNG sizes in browser and native document tests.
  Optional `apace-accesskit` connects the live semantic tree and Focus/Click/SetValue
  actions to desktop AccessKit. Virtual-row ScrollIntoView uses the shared list
  controller, then native ancestors/viewport, preserving the editor and its focus.
  Windows UI Automation and macOS AX have real client evidence;
  password values stay out of the tree, and list exports remain windowed.
  Linux AT-SPI clients, screen-reader announcements, remaining platform IME and
  Android system integrations still need their own acceptance; see the [current boundary](crates/apace/README.md).

External resource loading policies, platform accessibility acceptance, complete
Apace OS input acceptance and Android host integration remain in progress. DOM
behavior has automated Chromium evidence; native window test scope and remaining
OS/device requirements are recorded separately in the implementation ledger.

Optional phase timing uses `render::profile::Profile::native()` and
`app.set_profile(Some(profile.clone()))`; browser apps can call
`web_app.enable_profile()?`. Snapshots retain 31 aggregate phases without a
frame history. Timings include nested work and must not be added together.
Use `None` to disable profiling, or `profile.reset()` for a new interval.
The `semantic_frames` UI bench and Apace `profile` example provide release
baselines; they do not report browser internals, presentation waits or FPS.

Native hosts can call `app.settle(8)?` before drawing to consume bounded local
geometry feedback. Its `pending` flag requests another host turn when the
budget runs out; it does not wait for I/O or browser RAF. WindowApp drives this
automatically, with a separate budget for local tasks.

The [component contract audit](docs/semantic-ui-contract.md) records shared
content/event/query/action rules and backend differences. Group has no physical
query target; valid unsupported queries still complete asynchronously. Adapter
tests can opt into `glory-render/conformance` for the shared target matrix.
Typed event validation also checks local geometry and embedded origins before
handlers. A malformed event does not discard later edits/query responses from
the same drained tick; valid writes still flush before the error is reported.
Input values use LF/U+FFFD canonically while business Cages remain untouched.
Edits and `Commit { value, revision }` carry a program barrier; bound writes
follow the live node's version after explicit writes. Email/Url use text controls
with keyboard hints, preserving literal strings and selection.

## Reactive runtime

```rust
use glory::{Runtime, core::AccessError};

fn main() -> Result<(), AccessError> {
    let runtime = Runtime::new();
    let scope = runtime.scope();
    let count = scope.cage(1_u64);
    let parity = scope.bond(move || count.get() % 2).with_partial_eq();

    scope.binding(move || println!("parity: {}", parity.get()));
    runtime.flush()?;
    runtime.batch(|| {
        count.revise(|value| *value = 2);
        count.revise(|value| *value = 3);
    })?;
    // Parity stays 1, so its consumer does not run again.
    Ok(())
}
```

`Cage` and `Bond` are Copy local handles. Values and subscriptions belong to an
Owner; dropping a scope invalidates its handles and cancels owned work. A bare
read does not subscribe a component. Bond mappers, bindings and effects create
explicit tracking contexts.

`model.field("title", |m| &m.title, |m| &mut m.title)` returns an owned Cage
alias of a fixed inline field. Its writes notify whole-model/ancestor readers
and related field aliases, leaving unrelated field computations alone. Root
writes still notify every registered field. Aliases follow component Owners
and empty paths are reclaimed; accessors must be pure and select stable inline
storage. This does not replace batching or imply value equality.

For collections, `rows.item(index)` tracks a Vec position and `rows.entry(key)`
tracks a BTreeMap/HashMap key. Neither clones the source or its values or caches
element addresses. `try_with`/`try_revise` return `AccessError::MissingField`
while absent; missing reads remain subscribed and recover after reinsertion.
Use stable business keys for row identity, and include a generation when key reuse
means a new entity. Arbitrary `revise` conservatively notifies all live paths. Paths and
key buckets follow Owners, without visited-row history. Key hashing/comparison
must be pure; Deref and arbitrary custom container projections remain pending.

Explicit collection edits avoid unrelated path work: map `insert`/`remove` target
one key; Vec `push`/`pop`/`insert`/`remove`/`truncate`/`clear` target changed
positions. They also offer `try_` forms, create no item/entry handles, and publish
nothing for known no-ops. Invalid Vec indices return `OutOfBounds`; impossible
length growth returns `Capacity` (allocator/user panics retain std behavior).
Use `revise` for arbitrary or broad compound edits. Vec middle insertion/removal
still moves business elements; windowing does not make that data operation O(1).

Field/collection writes reuse one notification target per root. Switching paths
reuses its buffers; alias membership changes invalidate it. Targets contain only
generational identities and the latest logical selection, and every write still checks/increments current versions
and uses current subscribers. `RuntimeStats::field_target_*` expose preparations
and live cache identities. They do not measure total memory or presentation time.
The optional numeric collection selector stays in the root cache slot, outside
ordinary field-write arguments/guards, and is reclaimed on topology invalidation.
Repeated selection of the same logical field/item/entry in one Owner reuses its
alias. Event-created values belong to the listener's registering scope; observer
callback creations belong to the observer's Owner. Neither prolongs that Owner.

Use `scope.request(query, reply)` for component-owned host observations;
`app.request` belongs to the application root. Replies run under that Owner,
and retiring a requester releases its waiters even if the target stays live.
Cleanup callbacks run untracked in their retired Owner, so implicit construction
cannot escape into the caller. Explicit entry into another live Owner is allowed.
`Runtime::enter(instance, callback)` exposes the checked synchronous entry for
hosts; normal application handlers receive this context automatically.

`get()` returns an owned clone. Use `with(|data| ...)` for large collections to
avoid full-data copies. A windowed list accesses its render range and at most
one focused source key. The [list baseline](docs/semantic-ui-list-baseline-2026-10-21.md) separates
algorithm/lifecycle evidence from browser frame-rate claims.

Tasks require an injected `core::Host`: spawn queues local futures and provides
actual cancellation, wake and clock services. There is no synchronous executor
fallback. Derived values and prop bindings are pure; effects can write and
schedule another stabilization pass.

`Owner::resource(source, load)` owns keyed async state and cancels obsolete
requests. `Resource` is a Copy handle with tracked reads/maps; terminal results
use `Rc` without requiring `T: Clone`. `refresh()` and `cancel()` immediately
revoke old publication rights. `ResourceSeed` restores an explicitly decoded
matching terminal value. The optional `resources` feature adds an explicit
named JSON registry for transfer. Native pull-driven streaming frames exist;
their browser progressive interpreter supports final-state hydration.
The browser resource fixture verifies that 100 million indexed rows keep at
most 15 active row requests in its 240px viewport and release all on unmount.

## Semantic components

```rust
use glory::prelude::*;

fn main() -> Result {
    let app = App::memory(); // Reference interpreter; no window or DOM.
    let count = app.scope().cage(0_u64);
    app.mount_with(|| col().fill((
        text().content(count.map(|value| value.to_string())),
        button().label("Add").on(activate, move |_| count.revise(|v| *v += 1)),
    )))?;
    Ok(())
}
```

`Widget::build` and control-flow factories run inside their component Owner.
Use `mount_with` for repeated root factories so maps of application-wide
sources belong to the mounted component. A map created outside a construction
context belongs to its source Owner. `glory::Scope` is the UI context;
`glory::core::Scope` is the underlying logical lifetime without presentation.

For large data, `list(count, key_at, |index, key| widget)` uses indexed access,
not a full collection snapshot. `index` is a reactive Cage. Bind changing item
data explicitly; the row factory runs once per mounted key. Ordinary `each`
renders the complete sequence and refreshes row item/index handles on changes.

Lists support a bounded physical surface, logical programmatic position,
viewport input and per-tick scroll/measurement coalescing. Use `.measure(24.0)`
for automatic heights with 24px estimates, `.measure_limit(1024)` to bound
retained height metadata, and `.locate(key_to_index)` for stable anchoring
across large prepend/reorder edits. Unmeasured/evicted heights remain estimates;
exact full-data heights require an application-owned index. Use `.revision(...)`
when indexed keys do not read reactive data. `.inspect(...)` reports window,
height storage and viewport metrics. `ListStats::pinned` counts extra mounted
rows outside the range (zero or one). Focused rows retain DOM/editing state and
tasks; blur or key deletion releases them. Indexed `.locate` preserves focus
across data edits; without it only the old index/current window can be searched.
`.row_limit(4096)` is the default window budget, including overscan; a focused
pin can add one row. Oversized initial windows fail before source/factory access.
An oversized later resize reports `RowLimit`, releases rows/tasks, and recovers
when the window fits again. Inspection retains its last successful statistics;
it is not an error-state live-row counter. Row cost remains application-defined.
Use `.name("Entries")` for the collection's accessible name. Tab enters the
list; unmodified arrows, Home/End and PageUp/PageDown on its root/row shell
navigate logical rows, including segmented far jumps. Tab into a row's native
controls and Shift+Tab back out remain native; editor keys keep their cursor
behavior. Focused-data deletion returns focus to the list. Rows expose logical
position/total through shared semantic metadata and HTML ARIA attributes.
Platform screen-reader acceptance and actual DOM frame costs remain pending.
`App::style_profile()` and `Scope::style_profile()` expose the selected host's
typed inline property/value/unit support. Unsupported creates and `StyleSet`
updates return `UnsupportedStyle` before changing the target model or command
queue; numeric validation remains separate. The reference host records styles,
while Web/SSR lower the current typed subset through the HTML mapper. CSS dialect
metadata does not validate arbitrary stylesheets or promise identical engines.
Maximum sizes use `Limit::{None, Px, Percent}`. `max_width(Limit::None)` removes
a stylesheet cap; leaving `max_width` absent lets that stylesheet apply. Preferred
and minimum sizes continue to use `Length`, including `Auto`.
Apace publishes its own `STYLE_PROFILE` (apace v3): typed sticky positioning and
automatic scroll anchoring return an explicit unsupported error until the pinned
engine implements their geometry. Disabling anchoring remains supported for
Glory's virtual lists. Raw author CSS retains the selected engine's coverage.
Queries flush preceding changes and validate the response against that accepted
commit, including delayed delivery after newer edits. Invalid watermarks, result
kinds or geometry complete the waiter with a typed error. Selection offsets use
UTF-16 code units across hosts. Commit receipts mean applied changes, not displayed
pixels; a nonadvancing receipt faults the App because its state is uncertain.
Run `cargo run -p glory --example semantic_counter`
for the current backend-independent construction example.

Enable the facade's `web` feature and use `glory::web::launch_with(root,
|scope| widget)` in wasm32. Keep its returned `WebApp` alive; dropping it
releases its DOM, listeners, observer and frame host. `root` must be empty.
Enable `ssr` for `glory::ssr::render(widget)` or `render_document`; `web` and
`ssr` can be enabled together. The `html` feature exposes typed platform
mapping options, e.g. `HtmlTextExt::html_options`, without adding DOM types
to UI components.

For adoption, render `glory::ssr::snapshot_with(|scope| widget)` on the server,
include `glory::html::BASE_CSS`, and insert its `.html` verbatim into an
exclusive mount element. Enable `hydrate` and call
`glory::web::hydrate_with(root, |scope| widget)` with the same initial data.
Version/structure mismatches return `HydrationError` and preserve the existing
subtree. Existing form properties are delivered through normal semantic events,
with focused controls/selection preserved. Measured lists preview in flow and
adopt their existing visible row before switching to bounded window positions.
The plain HTML snapshot contains no arbitrary application values. Enable
`resources` for `Resources::bind(owner, name, schema, source, load)`, then use
`ssr::snapshot_resources_with` to export live terminal values before App cleanup.
Embed its `.view.html` in the mount and `ssr::resource_script(id, &data, nonce)`
outside it. The client uses `web::read_resources` and `Resources::restore`, then
builds the same named resources during hydration. A matching success/error seed
skips the first request; refresh/new keys load normally. Create a new context per
request/app and restore a fresh context from the original payload on retry.

The [browser fixture](examples/semantic-web) verifies 100 million lazy rows,
bounded DOM/graph slots, segmented scrolling, form updates, range movement,
Runtime isolation and cleanup. Its evidence is recorded in the
[implementation ledger](docs/semantic-ui-implementation.md); it is not an FPS
benchmark or an OS keyboard/input-method test.
The measured-row scenario is `examples/semantic-web/adaptive-acceptance.js`;
the original `acceptance.js` continues to cover fixed rows and forms.
`hydrate-acceptance.js` uses `/ssr.html`, generated by the native `ssr_fixture`
binary from the exact component factory used by the wasm application.
`resource-snapshot-acceptance.js` uses native `/resource-ssr.html` with CSP,
safe raw-text JSON, zero initial loads, schema/type diagnostics and retries.

The native `stream` feature exposes `ssr::stream_with(resources, config,
|scope, resources| widget)`. Its local body implements `futures_core::Stream`
and `next_frame()`: Shell arrives before loader polling, Patch carries shared
mapper edits, End carries terminal resource data and canonical key relabeling.
Pausing demand pauses rendering; dropping the body cancels request work. The
`stream_fixture` binary and `scripts/check-semantic-stream.py` verify real HTTP
arrival order, disconnect cleanup and a paused TCP reader using NDJSON. The same
fixture serves live HTML at `/page` and reversed completion at `/reverse`.
Serve `web::STREAM_SCRIPT`, emit `bootstrap.head_script()` before the shell,
then write each `frame.to_html(&bootstrap)` in demand order. External scripts
run after complete inert JSON blocks; configure CSP/nonce in the HTTP host.
After `glory:stream-ready`, call `web::hydrate_resources_with(root, &data,
limits, factory)` with the same final-state factory. It preserves the payload
on failure, removes it on success, and returns `(WebApp, Resources)`.
`stream-acceptance.js` verifies real progressive DOM, input/selection,
composition across bootstrap and adoption, range reuse, CSP and list scrolling.
The connection-thread HTTP implementation is a fixture, not a production server.

## Checks

On Windows PowerShell:

```powershell
./scripts/check-semantic-ui.ps1
./scripts/check-semantic-ui.ps1 -Bench
./scripts/check-semantic-ui.ps1 -Web
./scripts/build-semantic-web.ps1 -Serve
```

The script places Cargo home, target output and compiler temporary files under
`D:/BuildCache/glory-semantic-ui`, checks the actual target directory with
Cargo metadata, and runs formatting, active-workspace tests and strict Clippy.
Engine modifications live in the [glory-rs Apace/Takia forks](docs/engine-forks.md),
with full Git revisions and fork-owned tests checked by the same script.
Optional native profiling splits engine style, construction, box layout and
text work into bounded aggregate counters. After building the raster `profile`
example in release mode, `scripts/check-semantic-profile.py --binary <D: binary>
--out <new D: BuildCache directory> --quick` compares instrumentation on/off
without changing the workload. Inclusive phase totals overlap; these offscreen
CPU measurements do not establish window presentation FPS.
For a preserved before/after build use `scripts/compare-semantic-profile.py` with
`--before`, `--after` and a new D: `--out` directory. Add `--scroll` to exercise
17px programmatic physical scrolls; default seeks move the logical window.
Use separate target directories for the two builds. The example verifies real
24/28/32px row heights and compares final pixel sequences outside the timer.
Its default measurement budget retains the current window; `--measure-limit N`
can exercise larger caches, whose eviction may change later estimated windows.
The active workspace is explicitly selected in Cargo.toml. Standalone examples
and benchmarks require their own documented build and acceptance commands.

Rust 1.94 or later is declared. Current verification uses Rust 1.98; foundation
checks also cover wasm32-unknown-unknown, aarch64-linux-android and
x86_64-unknown-linux-gnu. Actual platform UI validation is tracked separately.

The [Glory/Sapid WASM size baseline](docs/wasm-size-baseline.md) measures minimal apps, unused features and actual widget use, with browser checks and a regression gate.
