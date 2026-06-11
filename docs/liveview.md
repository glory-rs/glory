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
tree on a dedicated local session thread and forwards WebSocket messages across
a channel, preserving `CommandHolder`'s single-threaded `Rc`/`RefCell` model
while satisfying Salvo's `Send` WebSocket task boundary.

## Current Boundary

The crate defines protocol/session behavior and tests command patches after
events. `LIVEVIEW_CLIENT_JS` provides a small reconnecting browser WebSocket
client that expects the command interpreter to be loaded first. The interpreter
now exposes `window.__gloryWryEvent` and `window.__gloryWryQuery` hooks so the
client can forward DOM events and node query answers over WebSocket.

Salvo is the first first-party route adapter. Axum and Actix should remain thin
wrappers around the same `LiveViewSession` and `LiveViewMessage` contract.
