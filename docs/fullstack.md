# Glory Fullstack Notes

Glory server functions are enough for small fullstack apps without a separate
REST layer: write `#[glory::server] async fn`, mount the adapter router, and
call the function from widgets.

## Server State And Cache

`glory-serverfn` exposes two process-local helpers for examples and small
deployments:

```rust
static TODOS: std::sync::LazyLock<glory_serverfn::ServerState<Vec<Todo>>> =
    std::sync::LazyLock::new(|| glory_serverfn::ServerState::new(Vec::new()));

static USERS: std::sync::LazyLock<glory_serverfn::ServerCache<String, User>> =
    std::sync::LazyLock::new(glory_serverfn::ServerCache::new);
```

- `ServerState<T>` stores a versioned value and supports `get`, `set`, and
  `update`.
- `ServerCache<K, V>` supports `get`, `put`, `get_or_try_insert_with`,
  `invalidate`, `invalidate_all`, optional TTL, and a version counter.
- Both are process-local. Use a database or distributed cache when several
  server processes must share state.

## SSR Preloaded State

`PreloadedState` is a JSON state bag intended for SSR handoff:

```rust
let mut preload = glory_serverfn::PreloadedState::new();
preload.insert("todos", &todos)?;
let script = preload.script_tag("__glory_state")?;
```

The generated script uses `type="application/json"` and escapes JSON for safe
embedding in HTML. Hydrated clients can parse the same JSON before falling back
to a server-function fetch.

## Streaming, SSE, And Uploads

`glory-serverfn` has adapter-agnostic helpers for custom resource routes:

```rust
let response = glory_serverfn::StreamingResponse::sse([
    glory_serverfn::SseEvent::named("todo", "created").id("42"),
]);
assert_eq!(response.content_type(), glory_serverfn::SSE_CONTENT_TYPE);
```

- `StreamingResponse` wraps a content type, optional headers, and a boxed byte
  stream for non-wasm custom routes.
- `SseEvent` encodes Server-Sent Event frames, including `id`, `event`,
  `retry`, comments, and multiline `data`.
- `encode_json_line` / `decode_json_lines` cover complete NDJSON-style resource
  streams.
- `NdjsonDecoder<T>` and `SseDecoder` incrementally decode streamed chunks on
  the client side. Hydrated clients can render SSR `PreloadedState` first, then
  feed later `fetch`, reqwest, or WebSocket chunks into these decoders without
  changing the resource format.
- `TransportMessage<T>` and `WebSocketFrame` provide framework-neutral typed
  envelopes for WebSocket/SSE/IPC transports:

```rust
let msg = glory_serverfn::TransportMessage::data("created");
let frame = glory_serverfn::WebSocketFrame::text_json(&msg)?;
let decoded: glory_serverfn::TransportMessage<&str> = frame.decode_json()?;
assert_eq!(decoded, msg);
```

Multipart uploads can be decoded with explicit limits:

```rust
let form = glory_serverfn::decode_current_multipart(
    request_body,
    glory_serverfn::MultipartLimits::default(),
)?;
let title = form.text("title")?;
let file = form.file("avatar");
```

The parser handles fields, files, filenames, per-part content types, body size,
field size, file size, and part-count limits. Adapter mounts still expose
server functions as JSON/form endpoints; wire `StreamingResponse` and multipart
helpers from custom Salvo/Axum/Actix routes when a route needs chunk flushing
or file bodies.

See [server function adapter recipes](serverfn-adapter-recipes.md) for concrete
Salvo, Axum, and Actix route snippets covering SSE/NDJSON streaming, multipart
uploads, and login/logout cookie redirects.

## Runnable Example

See `examples/todomvc-fullstack` for list/add/toggle/clear server functions,
SSR and CSR entry points, and request-context cookie display.
