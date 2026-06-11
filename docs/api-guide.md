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
cargo run -p glory-cli -- --manifest-path examples/ssr-simple-salvo/Cargo.toml check
cargo run -p glory-cli -- --manifest-path examples/ssr-simple-salvo/Cargo.toml bundle --release
```

`glory end2end` runs the `package.metadata.glory.end2end_cmd` configured by an
example or app. The first-party Playwright projects live in `tests/playwright`.
