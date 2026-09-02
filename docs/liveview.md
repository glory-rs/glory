# LiveView Protocol

`glory-liveview` is the first server-held command-stream crate. It is deliberately
framework-neutral: Salvo, Axum, Actix, or another WebSocket stack can wrap the
same session type.

## Session Flow

1. The browser opens the WebSocket and sends
   `LiveViewMessage::Hello { protocol_version, resume_token }`.
2. If the token matches a live session, the server replies
   `LiveViewMessage::Resumed { token }` and keeps the existing browser DOM.
3. Otherwise the server creates a `LiveViewSession`, replies
   `LiveViewMessage::Session { token }`, and sends the initial
   `LiveViewMessage::Mount { commands }` batch.
4. The browser applies commands with the same command interpreter used by
   desktop.
5. Browser events are sent back as `LiveViewMessage::Event(EventData)`.
6. The server dispatches the event into the held `CommandHolder` and returns
   `LiveViewMessage::Patch { commands }`.
7. Node query answers use `LiveViewMessage::Query(QueryResponse)`.

## Message Shape

Messages serialize as tagged JSON:

```json
{"type":"hello","payload":{"protocol_version":1}}
```

Reconnects may include a resume token:

```json
{"type":"hello","payload":{"protocol_version":1,"resume_token":"lv-..."}}
```

The protocol includes `hello`, `session`, `resumed`, `mount`, `event`, `query`,
`patch`, `error`, `ping`, `pong`, and `close`.

## Salvo Adapter

With the `salvo` feature enabled, `glory-liveview` ships a WebSocket route:

```rust
use glory_liveview::salvo_mount;
use salvo::prelude::*;

let router = Router::new().push(salvo_mount::router(|| app()));
```

Use `salvo_mount::router_with_config` to override `LiveViewConfig`.

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

Use `axum_mount::router_with_config` or `axum_mount::route_with_config` to
override `LiveViewConfig`.

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

Use `actix_mount::scope_with_config` or `actix_mount::configure_with_config` to
override `LiveViewConfig`.

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
or a fixed root element into the protocol crate. Adapters provide convenience
route wiring, but user code owns the rendered shell.

## Session Lifetime

First-party adapters now keep a shared `SessionRegistry<SessionWorker>` per
mounted LiveView route. A fresh socket performs the hello handshake, receives a
session token, and owns a clone of the session worker handle while connected.
If the socket drops and the browser reconnects with that token before idle/TTL
expiry, the adapter resumes the same worker and does not replay the mount batch.

The worker pool defaults to `min(available_parallelism, 4)` OS threads and can
be overridden with `GLORY_LIVEVIEW_WORKERS`.

`LiveViewConfig` controls idle timeout, max lifetime, and the
`OutboundBuffer` coalescing cap. The adapters reap expired sessions
opportunistically on connect and on inbound client messages. They do not yet
bind resume tokens to authenticated users, persist sessions across server
restart, or run a background reaper while no traffic is flowing. Connection
age, authentication expiry, load balancer idle limits, and server shutdown
policy must still be enforced by the surrounding HTTP/WebSocket stack.

## Reconnect Backoff

`LIVEVIEW_CLIENT_JS` reconnects automatically after abnormal socket closure.
The default initial delay is `250ms`; `options.reconnectMs` overrides it. The
delay doubles after each failed close and is capped at `5000ms` by default;
`options.maxReconnectMs` overrides that cap. A successful `open` resets the
delay back to the initial value.

The client keeps the session token in memory for the lifetime of the page and
sends it in the next `hello` after reconnect. Pass `options.resumeToken` to seed
the first connection, and `options.onSessionToken(token)` to observe token
changes. The default client intentionally does not store tokens in
`localStorage`; cross-refresh persistence should be paired with application
authentication and explicit server policy.

Calling the handle returned by `window.__gloryLiveViewConnect(...).close()`
marks the client as closed and stops future reconnect attempts. A server
`close` message has the same stop-reconnect behavior.

## Current Boundary

The crate defines protocol/session behavior and tests command patches after
events. `LIVEVIEW_CLIENT_JS` provides a small reconnecting browser WebSocket
client that expects the command interpreter to be loaded first. The interpreter
now exposes `window.__gloryWryEvent` and `window.__gloryWryQuery` hooks so the
client can forward DOM events and node query answers over WebSocket.

Salvo, Axum, and Actix remain thin wrappers around the same `LiveViewSession`
and `LiveViewMessage` contract.
