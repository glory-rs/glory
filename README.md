<p align="center">
  <img src="assets/glory.svg" width="112" alt="Glory logo">
</p>

<h1 align="center">Glory</h1>

<p align="center"><strong>Reactive interfaces in Rust. One Widget contract, multiple rendering hosts.</strong></p>

<p align="center">
  <a href="https://glory.taidge.org/">Website</a> ·
  <a href="docs/README.md">Documentation</a> ·
  <a href="examples/counter/README.md">Try the counter</a> ·
  <a href="tools/cli/README.md">CLI guide</a>
</p>

---

Glory brings owned, fine-grained reactivity to browser interfaces, server-rendered
HTML and native applications. Write widgets with Rust builders or the optional
XML-like `view!` syntax, then select the rendering host in your application.

- **State with a lifetime.** `Cage`, `Bond` and `Vary` connect values to their
  consumers. Owners release bindings, callbacks and tasks when a widget retires.
- **HTML where you want it.** Raw elements compile to shared templates and dynamic
  slots. Higher-level widgets add typed control behavior through the same `Widget`
  interface.
- **Hosts with clear boundaries.** Browser DOM, SSR, Apace's native document
  engine and Wry's system WebView have documented capabilities and acceptance.
- **Tools owned by your project.** Build, run, watch, diagnose and bundle with a
  CLI pinned alongside your application dependencies.

> **0.7.0 release line · Rust 1.95+.** This series includes breaking API changes.
> Source development uses sibling Glory, Apace, Takia
> and Wingy repositories; see [engine setup](docs/engine-forks.md) and
> [release readiness](docs/release-readiness.md).

## Start building

Add Glory to your application:

```toml
[dependencies]
glory = { version = "=0.7.0", default-features = false }
```

For sibling-source development, add `path = "../glory/crates/glory"` to that
dependency. Registry consumers do not need the engine source checkouts.

Add the host features your application needs. `web` selects the browser host;
`ssr` enables server rendering; `hydrate` enables browser adoption of SSR output.
The [host table](#choose-a-host) covers native options.

### A widget in ordinary Rust

```rust
use glory::prelude::*;

fn counter() -> impl Widget {
    widget(|scope| {
        let count = scope.cage(0_u64);
        row().fill((
            text().content(count.map(|value| format!("Count: {value}"))),
            button().label("Add").on(activate, move |_| {
                count.revise(|value| *value += 1);
            }),
        )).build(scope)
    })
}

fn main() -> Result {
    let app = App::memory();
    app.mount(counter())?;
    Ok(())
}
```

`App::memory()` runs the reference interpreter without a window. The
[cross-platform counter](examples/counter/README.md) shows the browser, Apace and
Wry entry points for a shared widget.

### Or use the optional XML syntax

Enable `features = ["dsl"]` on the Glory dependency:

```rust
fn greeting() -> impl glory::Widget {
    glory::view! {
        <div>
            <span>"Hello, Glory"</span>
            <glory::ui::Button label="Save"/>
        </div>
    }
}
```

Raw HTML, typed widget tags and ordinary factories compose in one tree.
`Each`, `Show` and `Content` handle keyed lists, conditional branches and child
ranges. Both frontends use the same checked templates and ownership model;
`#[glory::component]` generates a typed `Widget` builder. See the
[display guide](docs/v2-display-keyed-content-guide-2026-10-28.md) and
[Widget DSL guide](docs/v2-widget-dsl-unification-2026-10-29.md).

## Run the example

From the Glory repository root, use the counter's project-local launcher.
On macOS or a configured WSL host:

```sh
./examples/counter/bin/glory doctor
./examples/counter/bin/glory serve --platform web
./examples/counter/bin/glory run --platform desktop
./examples/counter/bin/glory run --platform desktop --bin counter-wry
./examples/counter/bin/glory bundle --platform web
```

On Windows, use `./examples/counter/bin/glory.ps1` with the same arguments.
The launchers select the native tool target and inherit the caller's build paths; no global
Glory CLI is required. First use resolves the private runner and installs the
matching wasm-bindgen tool when needed.

The CLI also supports feature overrides, native application arguments, shell
completion, JSON build plans and opt-in WASM optimization. See the
[CLI guide](tools/cli/README.md) for setup and
[feature comparison](docs/cli-comparison-2026-11-02.md) for remaining gaps.

## Choose a host

| Target | Application features | Rendering path and scope |
|---|---|---|
| Browser | `web` or `dsl` | Incremental DOM updates, controlled input and browser event/frame handling. |
| SSR and hydration | `ssr`, `hydrate`; optional `stream`, `resources` | HTML snapshots, adoption and pull-driven progressive output. [Salvo](crates/salvo-adapter/README.md) supplies HTTP integration. |
| Native document | `apace`; optional `apace-raster`, `apace-shell`, `apace-accesskit` | HTML/CSS document layout, CPU rendering, window host and accessibility bridge. [Adapter guide](crates/apace/README.md). |
| System WebView | `wry` | Native Rust state and tasks with incremental IPC-driven HTML updates. [Adapter guide](crates/wry/README.md). |
| Android and iOS | App-owned native hosts | Activity/UIKit entries, packaging and platform setup live in the [mobile example](examples/mobile/README.md). |

Standard Widget hosts support mixed HTML/widget trees. The direct sparse
`glory::display::App` is a browser-only entry point. CSS, input, accessibility and
OS integration support vary by adapter; compilation alone does not establish
platform acceptance. Device installation, store distribution and screen-reader
speech remain separate verification tracks.

The HTML wire version is defined by [`FORMAT_VERSION`](crates/html/src/lib.rs).
Apace's typed style support is defined separately by
[`STYLE_PROFILE`](crates/apace/src/document.rs); these are independent contracts.

## Build on the core

| Area | What Glory provides | Start here |
|---|---|---|
| Reactive state | Owned state, derived values, field/collection projections and cancellable resources | [Runtime and host guide](docs/runtime-and-hosts.md) |
| UI and large lists | Typed controls, keyed rows and bounded fixed/measured-height windows | [API guide](docs/api-guide.md) |
| Routing | Typed targets, nested outlets, retained layouts, history and SSR outcomes | [Routing](crates/routing/README.md) |
| Localization | Opt-in Fluent catalogs and reactive application-local language selection | [i18n](crates/i18n/README.md) |
| Application widgets | Controls, forms, navigation, themes and specialized widgets built on Glory | [Sapid](https://github.com/sapid-ui/sapid) |

`get()` clones a value; use `with(...)` to borrow large data. Reactive reads track
inside bindings, derived values and effects. A bare widget-build read is not a
subscription. The [runtime guide](docs/runtime-and-hosts.md) explains ownership,
batching, resources, list identity, hydration and streaming in detail.

For production application builds, put release settings in the application's own
workspace manifest; dependency profiles do not apply to consumers:

```toml
[profile.release]
strip = "symbols"
```

## Develop and verify

The root runtime workspace, CLI and examples have separate checks. Lockfiles are
prepared locally, ignored by Git and retained for `--locked` verification.
Configure build/cache/temp storage for your host before direct Cargo commands.

```powershell
# Windows
./scripts/check-semantic-ui.ps1
./scripts/check-semantic-ui.ps1 -Web
```

```sh
# macOS; configures /Volumes/Data/Caches/glory
./scripts/check-semantic-apple.sh
```

WSL uses the [Linux check script](scripts/check-semantic-linux.sh) with a verified
D:-backed build root. Browser, native-window, IME, device and accessibility checks
are separate gates; the [implementation ledger](docs/semantic-ui-implementation.md)
records their scope and remaining work.

## Explore further

- [Documentation index](docs/README.md) — APIs, architecture and implementation guides.
- [Runtime and host guide](docs/runtime-and-hosts.md) — state, widgets, lists, SSR and hydration.
- [Semantic contract](docs/semantic-ui-contract.md) — events, queries, controls and adapter limits.
- [Performance reports](docs/performance.md) and [WASM size analysis](docs/wasm-strip-static-host-2026-10-30.md) — measured workloads and reproducibility.
- [Engine provenance](docs/engine-forks.md) and [release instructions](RELEASING.md) — source versions and publication requirements.

## License

[Apache-2.0](LICENSE). See [NOTICE](NOTICE) and
[licensing details](docs/licensing.md) for retained third-party notices.
