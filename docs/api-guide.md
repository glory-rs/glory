# Glory API Guide

Glory apps are plain Rust builder trees. There is no RSX/JSX macro:

```rust
use glory::web::events;
use glory::web::widgets::*;
use glory::{Cage, Scope, Widget};

#[derive(Debug)]
struct Counter {
    value: Cage<i32>,
}

impl Widget for Counter {
    fn build(&mut self, ctx: &mut Scope) {
        let value = self.value;
        button()
            .on(events::click, move |_| value.revise(|mut value| *value += 1))
            .text(self.value.map(|value| format!("Value: {value}")))
            .show_in(ctx);
    }
}
```

## Reactivity

- `Cage<T>` is the mutable state handle.
- `Bond<T>` derives a value from one or more cages/bonds.
- `Lotus<T>` accepts either a plain value or a reactive value.
- Reads inside `Widget::build` / `Widget::patch` subscribe the current view.
- Reads inside event handlers do not subscribe unless another tracking context
  is active.
- Use `reflow::batch(...)` around grouped writes when you are outside a CSR
  event callback.

Runnable examples:

```powershell
cargo check --manifest-path examples/counter/Cargo.toml --target wasm32-unknown-unknown
cargo check --manifest-path examples/todomvc/Cargo.toml --target wasm32-unknown-unknown
```

### App-Wide Reactive Context

`Truck` is app-wide typed context, not a signal. Reading a plain value from
`Truck` does not subscribe the current view, and mutating `Truck` does not
schedule patches. Store a `Cage` handle in `Truck` when context also needs to be
reactive:

```rust
use glory::{Cage, Scope, Widget};

#[derive(Clone, Copy)]
struct ThemeContext {
    theme: Cage<&'static str>,
}

#[derive(Debug)]
struct App;

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        ctx.truck_mut().inject(ThemeContext {
            theme: Cage::new("light"),
        });

        // Children can copy the Cage handle out of Truck. Later `.get()` or
        // `.map(...)` calls on the Cage use normal reactive subscriptions.
    }
}
```

Use this for app-wide state such as theme, locale, auth/session summary, or a
router handle. Keep per-component state in the component owner instead of in
`Truck`, and avoid global statics unless a measured app genuinely needs process
global state.

## Widgets

HTML tags are generated builder types:

```rust
section()
    .class("todoapp")
    .fill(h1().text("todos"))
    .fill(input().attr("placeholder", "What needs to be done?"))
    .show_in(ctx);
```

Common methods:

- `.fill(child)` appends a child widget.
- `.text(value)` writes text content.
- `.html(value)` writes raw HTML; sanitize user input before using it.
- `.attr(name, value)` writes attributes.
- `.prop(name, value)` writes DOM properties in CSR.
- `.class(value)` appends static or reactive classes.
- `.on(events::click, handler)` attaches an event handler.

The generated HTML/SVG/MathML surface has SSR and command-stream snapshots. Use
the public `web::widgets::svg` and `web::widgets::math` modules for SVG and
MathML builders.

Runnable examples:

```powershell
cargo test -p glory-core --lib --features web-ssr widgets::snapshot_tests::form_controls_render_expected_ssr_markup
cargo test -p glory-core --features backend-command --test command_backend markup_surface_conformance_via_command_stream
```

## Lists And Branches

Use `Each` for keyed list rendering:

```rust
ul().fill(Each::from_vec(items, |item| item.id, |item| li().text(item.title.clone())))
```

Use `Switch` when several branches share one placeholder and some branches
should be cached.

Runnable examples:

```powershell
cargo test -p glory-core --lib --features web-ssr widgets::snapshot_tests::each_large_random_shuffle
cargo check --manifest-path examples/counters/Cargo.toml --target wasm32-unknown-unknown
```

## Forms

Use event helpers for browser inputs:

```rust
let name = Cage::new(String::new());
input()
    .prop("value", name)
    .on(events::input, move |event| {
        name.revise(|mut value| *value = glory::web::event_target_value(&event));
    });
```

The `forms-showcase` example covers controlled and uncontrolled inputs,
checkboxes, radios, selects, file input, readonly state, and derived form
summary output.

Runnable example:

```powershell
cargo check --manifest-path examples/forms-showcase/Cargo.toml --target wasm32-unknown-unknown
```

## Routing

Enable the `routing` feature and mount a `BrowserAviator` for CSR routing:

```rust
BrowserHolder::new()
    .enable(BrowserAviator::new(route(), catch()))
    .mount(App::new());
```

For type-safe navigation, implement `Routable` on an app route enum and call
`goto_route` instead of formatting URL strings at each callsite:

```rust
use glory::routing::{
    AviatorExt, Routable, append_route_query_param, encode_catch_all,
    encode_route_param, parse_route_param, parse_route_query, query_param_or,
    required_query_param, split_catch_all,
};

enum AppRoute {
    Home,
    User { id: u64 },
    Search { q: String, page: u32 },
    Files { path: Vec<String> },
}

impl Routable for AppRoute {
    fn to_url(&self) -> String {
        match self {
            Self::Home => "/".to_owned(),
            Self::User { id } => format!("/users/{}", encode_route_param(id)),
            Self::Search { q, page } => {
                let mut url = "/search".to_owned();
                append_route_query_param(&mut url, "q", q);
                append_route_query_param(&mut url, "page", page);
                url
            }
            Self::Files { path } => format!("/files/{}", encode_catch_all(path)),
        }
    }

    fn from_url(url: &str) -> Option<Self> {
        let url = glory::routing::url::Url::parse(url).ok()?;
        let segments = url.path().trim_matches('/').split('/').collect::<Vec<_>>();
        match segments.as_slice() {
            ["users", id] => Some(Self::User { id: parse_route_param(id).ok()? }),
            ["search"] => {
                let query = parse_route_query(url.query().as_deref());
                Some(Self::Search {
                    q: required_query_param(&query, "q").ok()?,
                    page: query_param_or(&query, "page", 1).ok()?,
                })
            }
            ["files", rest @ ..] => Some(Self::Files {
                path: split_catch_all(&rest.join("/")),
            }),
            [""] | [] => Some(Self::Home),
            _ => None,
        }
    }
}

aviator.goto_route(&AppRoute::User { id: 42 })?;
```

`Aviator::back()` and `Aviator::forward()` expose history movement for backends
that support it. `BrowserAviator` uses `window.history`, while `MemoryAviator`
keeps an in-memory stack for non-browser hosts. Browser navigation scrolls to a
hash target or the top of the page by default; use `LocatorModifier::with_scroll(false)`
or a `noscroll` anchor attribute to leave scroll position alone.

Runnable example:

```powershell
cargo check --manifest-path examples/router-basic/Cargo.toml --target wasm32-unknown-unknown
```

## SSR And Hydration

SSR builds use `web-ssr`; browser builds use `web-csr`. Do not enable both in
the same target. A Salvo app usually mounts a `ServerHolder` through the
provided handler and compiles the browser leg separately.

Runnable examples:

```powershell
cargo check --manifest-path examples/ssr-simple-salvo/Cargo.toml --features web-ssr
cargo check --manifest-path examples/ssr-simple-salvo/Cargo.toml --target wasm32-unknown-unknown --features web-csr
```

## Server Functions

With the `server-fn` feature, `#[glory::server]` compiles into a server body and
a browser stub:

```rust
#[glory::server]
async fn list_todos() -> Result<Vec<Todo>, glory::serverfn::ServerFnError> {
    Ok(vec![])
}
```

Use `method = "GET"` for cacheable, read-only calls. Arguments are serialized
into the query string, while the default remains POST:

```rust
#[glory::server(method = "GET")]
async fn read_todo(id: u32) -> Result<Todo, glory::serverfn::ServerFnError> {
    todo_store().read(id).await
}
```

Mount the adapter router on the server side:

```rust
Router::new().push(glory::serverfn::salvo_mount::router())
```

Runnable examples:

```powershell
cargo test -p glory-serverfn
cargo check --manifest-path examples/todomvc-fullstack/Cargo.toml --features web-ssr
cargo check --manifest-path examples/todomvc-fullstack/Cargo.toml --target wasm32-unknown-unknown --features web-csr
```

## Styling

Plain classes remain the default. For local component CSS:

```rust
let scope = glory::web::scoped_css(":scope { display: grid; } button { color: red; }");
style().text(scope.css().to_owned()).show_in(ctx);
div().class(scope).fill(button().text("Save")).show_in(ctx);
```

See [styling notes](styling.md) for selector support and boundaries.

## Command Backends

Desktop, TUI, native experiments, and future LiveView use the command stream:

- `Command`
- `CommandQueue`
- `CommandHolder`
- `EventData`
- `NodeQuery`

The command stream is intentionally JSON-shaped today. See
[performance notes](performance.md) for wire-format and scheduler baselines.

Runnable examples:

```powershell
cargo test -p glory-core --features backend-command --test command_backend
cargo check --manifest-path examples/desktop-counter/Cargo.toml
```

## CLI

Important commands:

```powershell
cargo run -p glory-cli -- config --schema
cargo run -p glory-cli -- doctor --target web
cargo run -p glory-cli -- --manifest-path examples/ssr-simple-salvo/Cargo.toml serve --port 8080 --no-open
cargo run -p glory-cli -- --manifest-path examples/ssr-simple-salvo/Cargo.toml run --no-open
cargo run -p glory-cli -- completions powershell
cargo run -p glory-cli -- self-update
cargo run -p glory-cli -- --manifest-path examples/ssr-simple-salvo/Cargo.toml check
cargo run -p glory-cli -- --manifest-path examples/ssr-simple-salvo/Cargo.toml bundle --release
```

`glory serve` opens the resolved site URL in the default browser after the first
build. Use `--no-open` to suppress it, or `--address` / `--port` to override the
configured `site_addr` for the current run.

While `glory serve` is watching, stdin accepts line controls: `r` + Enter forces
a rebuild, `v` + Enter cycles log verbosity, and `/` + Enter prints the controls.

`glory end2end` runs the `package.metadata.glory.end2end_cmd` configured by an
example or app. The first-party Playwright projects live in `tests/playwright`.
