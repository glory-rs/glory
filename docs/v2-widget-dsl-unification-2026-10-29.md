# Unified Widget authoring in the XML DSL

`Widget` is the component contract for handwritten widgets, Sapid and function
widgets. The former display `Component`, `Component::Props`, `ComponentView`,
`IntoView` and `ChildContent` interfaces have been removed. `View` is template
configuration implementing `Widget`, rather than a second component interface.
`#[component]` remains a function-authoring macro and generates a Widget builder.

The sparse host now recognizes View's complete host path before type-erased
composition, using the internal `Widget::native_host` hint. Ordinary widgets
keep the default and implement `build` as before; Box forwards the same Widget
contract. Semantic islands are stored behind an internal flush/lifetime
interface, so a concrete raw-only app can drop unused browser-renderer code.
The [rendering and size measurements](framework-benchmark-render-optimization-2026-10-29.md)
cover the resulting artifacts, shared Each, node reuse and mixed-host checks.

Configuration actions and content installation select their internal host type
statically. Each/Show still use the same controller and Widget contract, while
raw-only programs can eliminate unused semantic mounting paths. Mixed apps keep
the necessary adapters. See the [2026-10-30 size and acceptance report](wasm-strip-static-host-2026-10-30.md).

## Consolidated implementation

The legacy `html::template::{Node, Target, Template}` AST/compiler and `Source`
dispatch have been retired. Handwritten declarations, DSL output and the sparse
browser host all use the checked `html::view::Template` compiler and its cached
immutable `Compiled` declaration. `html::template` now only contains the shared
slot kinds and finite-profile validation helpers. Named Children slots can append
dynamic children after static content; both stream validation and physical hosts
accept this same declaration shape.

Text/boolean sources expose one validation/read contract. Configuration actions
(`bind`, class flags, handlers and children) execute once through a small mount
adapter. `Each`, `Show`, lazy/optional/vector content use one controller for sparse
and standard hosts. Semantic `When` and display `Show` share branch selection and
retirement. `ui::Each` and `display::Each` now reexport the same source-generic
Widget/controller; see [Unified Each](v2-unified-each-2026-10-29.md). An
optional Widget uses the same composed-child retention as factory widgets, keeping
build resources alive until its Owner retires. Reactive text/class bindings read
computed sources once on their initial binding tick, then once per dependency tick.

Some adapters deliberately remain: standard hosts have semantic outlet anchors,
while sparse browser mounts own detached ranges. The unified Each adapts
`Vary<Vec<T>>` item/index cages and generational `KeyedList` members without
converting their storage. These source and placement policies share one
component contract and reconciliation controller. Browser create,
hydration adoption, SSR serialization, incremental wire edits and native physical
node/AX operations also retain their necessary target-specific implementations.
The classic stream JS validates untrusted wire data independently of Rust.

Sapid's `docs/dsl.md`, `examples/fixtures/dsl_demo.rs` and
`scripts/check-dsl.ps1 -Browser -Native` show and verify one application Widget
compiled unchanged for SSR, browser creation/adoption and native layout/input.
Its default library dependency graph still has no DSL compiler or host dependency.

The consolidation was checked with the hand/DSL browser comparison, 600-row
ownership/GC checks, seeded 10k-row oracle, sparse density and failure-prefix
probes, mixed CSR/SSR adoption, gated incremental HTTP streaming and native
layout/input/CPU painting. Density still reports 2 versus 201 physical nodes with
the same one instance, two targets and one record. The native engine suite passes
165 tests (two platform-specific cases remain ignored). Downstream compiler
checks now cover 29 current Widget/factory/property cases, with no legacy
Component or Source fixture. These are correctness checks, not new timing claims.

```rust,ignore
use glory::display::prelude::*;
use sapid::Button;

#[component]
fn Counter(scope: &Scope, initial: usize) -> impl Widget {
    let count = scope.cage(initial);
    view! {
        <div>
            <span>{count}</span>
            <Button label="Increment" on_press={move || count.revise(|n| *n += 1)}/>
        </div>
    }
}

let app = App::launch("main", Counter::default().initial(0))?;
```

## Construction and children

- An unqualified lowercase tag such as `<button>` is raw HTML, independent of
  imported builder functions. It still emits the shared sparse template.
- Capitalized tags and Rust paths name types. `<Button>` follows normal Rust
  imports and aliases; `<sapid::Button>` is explicit. Ambiguous imports are
  Rust compile errors; the macro does not search dependency crates.
- A tag starts with `Type::default()`. Named attributes call builder methods
  in source order. A bare attribute calls a zero-argument method, e.g. `unstyled`.
  Boolean setter inputs use `enabled={false}`. Method names and argument types
  remain checked by Rust.
- A type requiring construction arguments uses `init`, e.g.
  `<sapid::ActionIcon init={sapid::action_icon("Refresh")} icon={icon}/>`.
  The initializer is checked against the named type. This is still an ordinary
  Widget; no DSL registration or additional component trait is required.
  Type aliases share their underlying type's Default; alternative factories
  such as `dropzone()` also use an explicit initializer to select that behavior.
- Child tags call `.fill(defer(move || ...))`. Function widgets declare
  `children: glory::ui::Children`, exactly the collection used by Sapid builders.
  They can forward it through `<Content value={children}/>` or `.fill(children)`.
  `Content` also accepts any Widget or a collection implementing IntoChildren;
  use `fragment(widgets)` when returning a Vec of widgets from a Rust function.
  Required function properties use builder type states: missing properties do
  not implement Widget and fail at compile time. Property setters take their
  declared concrete types, preserving integer inference and ordinary Rust errors.
- The injected `scope: &Scope` in a function widget is the reactive core Scope.
  Its body executes once inside the mounted owner. Handwritten `Widget::build`
  continues to receive the semantic UI Scope, which dereferences to that core
  scope. Signal changes update bindings rather than rerun function widgets.

## Runtime integration

The standard hosts accept the same `impl Widget`: `ssr::snapshot`,
`web::launch` / `web::hydrate`, and `blitz::shell::WindowApp::mount`. This includes
HTML/Widget/HTML nesting, Sapid, function widgets, Show and keyed Each.

Each template has one owned Group boundary and each content slot has a Group
range. Raw HTML elements do not acquire semantic Memory entries or Mapper nodes.
Static declarations are compiled once and shared; current text/attribute/class
slots belong to the instance. Dirty instances enter a weak renderer queue, so
field updates do not scan every live instance. The renderer applies these slots
before physical actions and queries. Widget outlets remain in the same semantic
tree and reactive Runtime as their surrounding application.

SSR writes escaped physical HTML and versioned instance/outlet/text boundaries.
Snapshot format is v14; server and browser builds must agree. No runtime NodeId
is serialized for raw event targets. Hydration validates the complete tree,
including static attributes, before adopting targets or installing handlers.
Empty/adjacent text slots retain separate markers. Existing controlled Widget
editor adoption preserves dirty values, identity and selection.

Native creates the same physical template tree in Blitz. Dirty slots update
those physical targets directly. Raw buttons support pointer, Enter/Space and
Tab; Widget controls retain their existing typed input protocol. Native
AccessKit traverses raw nodes and semantic outlet ranges in physical order,
with physical AX identities rather than per-element semantic identities.

`display::App` remains the browser-only sparse fast path and retains its existing
Widget-island bridge. Use the standard hosts above for a portable mixed tree;
they do not add a `display:contents` wrapper around each Widget island.
Owned scopes release nested templates, bindings, events and captures together.
Raw content inside a semantic Button/Link is restricted to text, fields and spans
without dynamic outlets. HTML context checks also protect table outlets.

## Function factory tags

An existing function needs no attribute macro or registration:

```rust,ignore
fn button(label: &str) -> impl Widget {
    sapid::button().label(label.to_owned())
}

view! {
    <div>
        <button>"HTML"</button>
        <sapid::Button label="type builder"/>
        <{button("opaque return type")}/>
        <{sapid::button()} label="concrete return type"/>
        <{sapid::card()}><span>"children"</span></>
    </div>
}
```

`<{expression}/>` evaluates the Rust expression once in its mounted owner.
Arguments, module paths and generic calls use ordinary Rust syntax. With children,
the anonymous expression tag closes with `</>`. Setters/children work when the
expression's return type exposes those builder methods. A return type written
only as `impl Widget` hides concrete `.label` / `.fill` methods; pass those values
as factory arguments, configure inside the factory, or return the concrete type.
The DSL does not infer hidden methods. `<button>` always remains raw HTML, even
if a function named `button` is imported. `<Button>` uses normal Rust type imports.

## Target boundaries and verification

The finite raw HTML profile still applies: div/span/button/a/h1/table/tbody/tr/td,
text, declared attributes, click and typed slots. Controlled editors use Widget
tags. This does not add arbitrary HTML, all ARIA/native semantics, or OS IME
support. Synchronous snapshots, incremental SSR and final adoption use the same
template/Widget contract and HTML v14. Serve the matching `STREAM_SCRIPT` asset.

`examples/display-web/src/mixed.rs` is compiled unchanged for real server HTML,
browser CSR/hydration and Sapid's native acceptance. Run
`scripts/build-mixed-display.ps1 -Verify` for the browser fixture, and the sibling
`sapid/examples/native` test `mixed_dsl` for engine layout/input/CPU raster checks.
The browser checks include dirty pre-hydration input/selection, tampered markup
refusal, empty/adjacent text, table outlets, raw/Widget events, reactive attributes
and classes, keyed identity/focus, branch capture release and bounded cleanup.
Native AccessKit order/actions/focus/retirement are checked separately in
`crates/blitz/src/accessibility_display_tests.rs`. These are browser/engine/AX-tree
checks; they do not establish OS screen-reader announcements or device behavior.
No timing or WASM-size improvement is claimed by this migration.

## Incremental SSR

`ssr::stream_with` accepts the same `view!` result, including Sapid and
`<{factory(args)}/>` tags. The existing pull-driven Shell/Patch/End envelope and
`Bootstrap` API remain the transport interface; no separate component trait or
stream-specific DSL is needed.

Root shapes carry a declaration key, current typed slot values and (on the first
live reference) the immutable compiled declaration. Other instances share it;
last-instance disposal retires its cache entry on both server and browser. A
later instance can define the same static template again without retaining
visited-instance history. Outlet shapes reference their owning template key and
slot. Raw elements still have no individual semantic identities.

The renderer drains only dirty template instances at the UI commit barrier.
Text, attribute and boolean class changes coalesce into `display` edits; Widget
controls continue using the existing typed edits. These patches share per-frame
byte limits and cancellation/backpressure with semantic operations. Slot-only
updates do not manufacture semantic mutations or recreate template roots.

The classic CSP-compatible bootstrap validates finite declarations/slots,
constructs detached template trees, adopts the parsed shell, routes Widget
outlets and preserves dirty controls during moves and suspension. It executes no
application callbacks before WASM adoption. End rekeys both semantic boundaries
and template outlet comments, then releases stream caches. Full Widget hydration
can adopt the resulting DOM with its dirty input values and selection intact.

Run `scripts/build-mixed-stream.ps1 -Verify`. The shared
`examples/display-web/src/stream_mixed.rs` is compiled for a real Rust streaming
response and browser hydration. Its test host gates three server stages rather
than replacing HTTP with generated JS operations. Checks cover fields/classes,
named Children and range outlets, HTML/Widget nesting, table Each insertion,
deletion/reordering, Show retirement/recreation, initially suspended templates,
offline updates/resume, dirty inputs, fallback movement/focus/selection, final
rekey/adoption/events, composition-delayed End, retired-DOM garbage collection,
malformed payload rejection and disconnect cleanup.
Rust regressions additionally verify declaration reuse/retirement, field
coalescing without semantic commands, and byte-limit cancellation of pending tasks.
