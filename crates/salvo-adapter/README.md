# glory-salvo

Native Salvo 1.0 integration for the current semantic UI and `glory-ssr`.
This package is an active workspace member. It replaces the former
`ServerHolder` adapter; no legacy HTML API or feature aliases are retained.

## Ordinary SSR

`render_response(widget)` and `render_response_with(|scope| widget)` return
`glory_ui::Result<salvo::Response>`. Construct reactive state with
`scope.owner().cage(...)` inside the factory. Rendering is synchronous and
request Owners are cleaned up before returning. Async data must be prefetched
or explicitly seeded; this path does not wait for loaders.

`document_response(widget, &ssr::Document)` renders a complete document.
`html_response(html)` wraps an already assembled string, including versioned
`ssr::snapshot*` output. All helpers set HTTP 200 and
`Content-Type: text/html; charset=utf-8`; rendering failures return a typed error
before producing a response. The application chooses its error status/body.

Optional `css` forwards Sheet support to SSR. Optional `resources` exposes
`ssr::snapshot_resources_with` and `ssr::resource_script`: export terminal data
while its request App is alive, and place the escaped inert resource script
outside the exclusively hydrated root before wrapping it in `html_response`.

## Progressive SSR

Enable `stream`. Share a `StreamRenderer::new(max_requests)?` across routes,
then call `renderer.render(StreamPage { document, bootstrap }, factory).await`.
The factory returns `Result<ssr::stream::Stream, ssr::stream::Error>` and calls
`ssr::stream_with(Resources::default(), config, |scope, resources| widget)`.
Create fresh Resources, reactive state, widgets and local futures **inside**
that factory. Capture only owned Send request inputs; never capture a live
Scope, Cage or Resources from a Salvo handler.

Each admitted request owns one native worker thread and a current-thread Tokio
runtime, so ordinary Tokio timers/network I/O can wake its local resource
futures. The UI never crosses threads. Admission is bounded with no wait queue;
`StreamError::Busy` can be mapped to HTTP 503. Cloned renderers share the limit.
Choose both that limit and the underlying SSR task/chunk/resource budgets for
the application. Separate renderer instances have separate limits.

The response is prepared after shell construction/serialization, before polling
any loader. It starts with the shared SSR document prefix and preload capture
script, then the Shell. Subsequent HTTP body polls request one Patch/End at a
time; no background producer collects or renders the remaining frames. The
bridge has at most one outstanding demand/reply, with no frame backlog. TCP
and server buffers may still accept a finite prefix while a client pauses.

Dropping the response/body cancels idle and pending-I/O requests, disposing the
App, tasks and resource registrations on their worker. It never blocks the
network thread waiting for a join. Cancellation is cooperative: synchronous
application code must return/yield; blocking factory/loader code cannot be
preempted. Request admission is released after local cleanup and runtime drop.

Startup failures return `Err` so a handler can select an HTTP error response.
Later rendering/serialization failures become body errors, closing the response
without a successful End. Worker panics close outstanding replies and also
terminate the body; they are not rendered into the page.

Serve the **matching** `glory_web::STREAM_SCRIPT` at `bootstrap.script_url` and
allow it in the application's CSP. The preload script precedes editable shell
DOM; every frame uses the shared inert JSON + parser-blocking external script
format. End carries canonical rekeying and terminal resource seeds for an
optional later WASM hydration. This adapter does not invent another wire format
or automatically initialize WASM. CSP headers and any hydration assets belong
to the application. `Document::nonce` covers its styles, while
`Bootstrap::nonce` covers the stream scripts; configure both under your policy.

Responses set `Cache-Control: no-store` and `X-Accel-Buffering: no`, without a
Content-Length. Configure middleware, compression and reverse proxies to flush
progressively; the header alone cannot control every intermediary.

## Runnable example and checks

The active [example](examples/ssr.rs) serves `/plain`, `/stream` and the matching
`/stream.js`. `/stream` delivers an immediate shell and two independently
delayed resource updates. Its CSP allows same-origin external scripts and
inline styles; no inline executable JavaScript is needed. It demonstrates
progressive DOM updates and End resource export, without a WASM bundle.

```powershell
$env:CARGO_HOME = 'D:/BuildCache/glory-semantic-ui/cargo'
$env:CARGO_TARGET_DIR = 'D:/BuildCache/glory-semantic-ui/target'
$env:TEMP = 'D:/BuildCache/glory-salvo/tmp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force $env:TEMP | Out-Null
cargo metadata --no-deps --format-version 1 # verify target_directory is on D:
cargo run --locked -p glory-salvo --example ssr --features stream
# Optional address: ... --features stream -- 127.0.0.1:5801
cargo test --locked -p glory-salvo --all-features
```

Tests cover semantic HTML/control escaping, reactive Owner cleanup, snapshot
wrapping, document metadata/CSS, resource seeds, first-byte ordering, per-demand
backpressure, reverse completion, request isolation, startup/body errors and
panics, cancellation during initialization, admission recovery, real Salvo
chunked HTTP, TCP disconnect and a paused TCP reader. The browser acceptance
script beside the example checks the actual progressive DOM and CSP behavior.

The library and example are native-only (empty on wasm32). Merely using the
Glory facade/default/HTML/SSR features does not enable this HTTP integration.
Other old Salvo application directories remain extraction references unless
explicitly migrated; use this active package's example for the new architecture.
