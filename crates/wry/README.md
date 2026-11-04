# glory-wry

A native Rust backend for Glory's semantic UI. Rust owns the App, Cage/Bond
state, callbacks and asynchronous tasks. A small JavaScript interpreter applies
typed HTML edits in the system WebView. This backend does not compile the UI to
WASM or replace its document with snapshots. Apace remains a separate backend.

Enable the facade's `wry` feature to select the Wry/Tao desktop shell:

```rust
use glory::{prelude::*, wry::shell::{Config, Shell, WindowApp}};

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let window = WindowApp::new();
    let count = window.app().scope().cage(0);
    window.mount(col().fill((
        text().content(count.map(|n| format!("Count: {n}"))),
        button().label("+1").on(activate, move |_| count.revise(|n| *n += 1)),
    )))?;
    Shell::new().run(window, Config::default())?;
    Ok(())
}
```

Run `cargo run -p glory --example semantic_wry --features wry`. The existing
Counter component also has a `counter-wry` binary; see its README. Use the
repository's approved cache environment before Cargo (on macOS, source
`scripts/apple-env.sh`). No global CLI or JavaScript build tool is needed.

## Current support

| Facility | Boundary |
| --- | --- |
| Basic UI | View, Group, Text, Button, Input/textarea, Check and Image/Link |
| Updates | Shared HTML create/field patch/place/suspend/resume/dispose, including detached Groups and explicit Plans |
| State | Native Cage/Bond and Owner lifecycle; one ready-task executor per WindowApp |
| Input | Activation, canonical text, controlled revision echoes, composition protocol, commit and checked state |
| Focus | Actual focus observations and queries, focus without scrolling, exact UTF-16 selection actions/queries |
| Geometry | Bounds, scroll offsets/metrics, ResizeObserver interests and scroll acknowledgements |
| Commands | Finite Keyboard/Navigate events on explicitly observed ancestors |
| Styles | Shared HTML inline profile, raw app CSS, optional validated `css` Sheet replacement |
| Resources | Explicit immutable `Config::assets` served by exact URL path; no implicit filesystem or remote-network access |
| Platform | Wry system WebView and Tao event loop on Windows/macOS/Linux; Linux uses Tao's GTK container for X11/Wayland |

Raw display/XML templates, structured Select, File, RichEditor, Graphic, virtual collections and advanced
interaction metadata (layers, focus scopes, motion, drag/swipe, choice/menu/tree
controllers, forms and clipboard services) currently return `Unsupported`.
Checks run in both staged preflight and direct application. They are not silently
advertised through capabilities. The HTML mapper retains its remount requirement
for physical tag changes.

`Renderer::set_author_css` updates the trusted application CSS layer (up to 1 MiB)
without remounting widgets. It uses the same ordered ready/ack transport and shell
CSP as the initial `Config::css`; CSS is installed as style text, never HTML.

Link activation can reach Rust handlers. The stock shell denies document
navigation and new windows, so opening external links is not a provided service.
Network I/O for application logic can use ordinary native Rust libraries; renderer
resources are local. System WebViews differ in CSS and HTML feature support;
identical pixels, OS IME gestures, screen-reader output and mobile support are not
established by this adapter's tests.

## Transport and lifecycle

`Renderer` can be used without `shell`. A custom host installs
`initialization_script()`, forwards IPC to `receive()`, sends `take_script()`
through its WebView and pumps the native App on its own event loop. Callbacks
enqueue only; business code runs during App observation. One document session
and monotonically ordered/acknowledged batches prevent stale windows from
addressing another consumer. Node addresses include runtime, slot and generation;
all u64 identities/revisions are decimal strings across JavaScript.

Queries retain their acceptance watermark across later native commits.
Responses validate type and finite geometry. Duplicate replies, retired nodes
and stale input revisions cannot call application handlers. Disposal retires
per-node pending queries; closing drops App before closing its executor and
revoking its document. DOM acknowledgement does not imply pixel presentation.

Limits are 1 MiB per incoming message, 8 MiB pending command/queued input payload,
4096 queued inputs, 1024 pending queries, and 32 MiB explicit shell resources.
Only one batch is in flight. The stock shell fails on document startup or batch
acknowledgement timeout after 15 seconds. These are transfer/admission limits,
not limits on WebView or OS allocation. The shell serves a local document with
CSP and blocks navigation; bootstrap scripts are explicit trusted application
code, not IPC commands.

## Verification

```sh
cargo test --locked -p glory-wry --all-features
cargo clippy --locked -p glory-wry --all-features --all-targets -- -D warnings
GLORY_WRY_WINDOW=1 cargo test --locked -p glory-wry --all-features --test window
```

The opt-in window test runs the actual system WebView on the UI/main thread.
It drives DOM input, verifies native Rust state and query callbacks, and checks
controlled Chinese/emoji text, UTF-16 selection, commit, checkbox, conditional
disposal/recreation, Group move/suspend/resume, task cancellation and asynchronous
native wakeups. Its injected DOM events are
protocol acceptance, not real keyboard/OS IME or assistive-technology acceptance.
macOS is the currently tested runtime; Windows/Linux runtime acceptance is open.
Windows cross-compilation has been checked; it does not establish WebView2
runtime behavior. The facade has also been checked with Wry and Apace enabled
together and on WASM without acquiring native WebView dependencies.

Text proposals use protocol v2 acknowledgements after the Rust UI turn. Each
live editor has one outstanding proposal; newer native typing remains in the
editor and its latest value follows the receipt. Controlled echoes preserve
that newer text. Explicit differing writes and rejected old program barriers
still replace it. Activation, commit and keyboard commands wait for preceding
text proposals, so handlers observe the latest accepted value. Deferred actions
are bounded and retired targets are discarded. Native window acceptance includes
a rapid-input burst followed immediately by activation, alongside the existing
UTF-16, composition, identity and lifecycle checks.
