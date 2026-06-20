# LiveView Protocol

`glory-liveview` is the first server-held command-stream crate. It is deliberately
framework-neutral: Salvo, Axum, Actix, or another WebSocket stack can wrap the
same session type.

## Session Flow

1. Server creates a `LiveViewSession` by mounting a widget.
2. The initial `LiveViewMessage::Mount { commands }` batch is sent to the
   browser.
3. The browser applies commands with the same command interpreter used by
   desktop.
4. Browser events are sent back as `LiveViewMessage::Event(EventData)`.
5. The server dispatches the event into the held `CommandHolder` and returns
   `LiveViewMessage::Patch { commands }`.
6. Node query answers use `LiveViewMessage::Query(QueryResponse)`.

## Message Shape

Messages serialize as tagged JSON:

```json
{"type":"hello","payload":{"protocol_version":1}}
```

The protocol includes `hello`, `mount`, `event`, `query`, `patch`, `error`,
`ping`, `pong`, and `close`.

## Salvo Adapter

With the `salvo` feature enabled, `glory-liveview` ships a WebSocket route:

```rust
use glory_liveview::salvo_mount;
use salvo::prelude::*;

let router = Router::new().push(salvo_mount::router(|| app()));
```

The route is mounted at `/__glory/liveview`. The adapter keeps the Glory widget
tree on a shared local worker pool and forwards WebSocket messages across a
bounded channel, preserving `CommandHolder`'s single-threaded `Rc`/`RefCell`
model while satisfying Salvo's `Send` WebSocket task boundary.

## Axum Adapter

With the `axum` feature enabled, mount the same session contract on an Axum
router:

```rust
use glory_liveview::axum_mount;

let app = axum_mount::router(|| app());
```

For an existing router, use the shared `LiveviewRouter` trait:

```rust
use glory_liveview::LiveviewRouter;

let app = axum::Router::new().with_liveview("/__glory/liveview", || app());
```

## Actix Adapter

With the `actix` feature enabled, mount either a `Scope` or configure an
`App`:

```rust
use glory_liveview::actix_mount;

let app = actix_web::App::new().service(actix_mount::scope(|| app()));
```

```rust
actix_web::App::new().configure(|cfg| actix_mount::configure(cfg, || app()));
```

Salvo, Axum, and Actix all use the same `LiveViewSession` and worker pool. The
framework modules only translate WebSocket message types and route builders.

## HTML Shell Ownership

`glory-liveview` does not own the page template. It only provides the command
stream protocol, the session object, the route adapter, and
`LIVEVIEW_CLIENT_JS`. The host application is responsible for serving an HTML
shell, including any `<head>` entries, root container markup, app-specific
assets, CSP nonces, and framework route layout.

A minimal shell must load the command-stream interpreter before the LiveView
client and then connect the WebSocket client:

```html
<script>
  /* command-stream interpreter, for example glory_desktop::WRY_INTERPRETER_JS */
</script>
<script>
  /* glory_liveview::LIVEVIEW_CLIENT_JS */
</script>
<script>
  window.__gloryLiveViewConnect("/__glory/liveview", {
    reconnectMs: 250,
    maxReconnectMs: 5000
  });
</script>
```

This keeps template policy in the HTTP framework layer instead of baking Salvo
or a fixed root element into the protocol crate. Future Axum/Actix adapters
should follow the same boundary: adapters may provide convenience route wiring,
but user code owns the rendered shell.

## Session Lifetime

Today, one accepted WebSocket connection owns one `LiveViewSession` running as a
local task on the shared worker pool. The session is created after the socket
upgrade succeeds, mounts the widget, sends the initial command batch, and is
dropped when the socket loop exits or the worker is otherwise closed.

The adapter-to-session channel is bounded at 32 pending messages, so slow
server-side widget work applies backpressure instead of growing an unbounded
queue. The worker pool defaults to `min(available_parallelism, 4)` OS threads
and can be overridden with `GLORY_LIVEVIEW_WORKERS`.

There is no protocol-level resume token, idle timeout, or session TTL setting
yet. Connection age, authentication expiry, load balancer idle limits, and
server shutdown policy must be enforced by the surrounding HTTP/WebSocket
stack. If resume support is added later, it should be explicit in the protocol
instead of silently reusing a stale `CommandHolder`.

## Reconnect Backoff

`LIVEVIEW_CLIENT_JS` reconnects automatically after abnormal socket closure.
The default initial delay is `250ms`; `options.reconnectMs` overrides it. The
delay doubles after each failed close and is capped at `5000ms` by default;
`options.maxReconnectMs` overrides that cap. A successful `open` resets the
delay back to the initial value.

Calling the handle returned by `window.__gloryLiveViewConnect(...).close()`
marks the client as closed and stops future reconnect attempts. A server
`close` message has the same stop-reconnect behavior.

## Current Boundary

The crate defines protocol/session behavior and tests command patches after
events. `LIVEVIEW_CLIENT_JS` provides a small reconnecting browser WebSocket
client that expects the command interpreter to be loaded first. The interpreter
now exposes `window.__gloryWryEvent` and `window.__gloryWryQuery` hooks so the
client can forward DOM events and node query answers over WebSocket.

Salvo is the first first-party route adapter. Axum and Actix should remain thin
wrappers around the same `LiveViewSession` and `LiveViewMessage` contract.
