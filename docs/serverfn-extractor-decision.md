# Server Function Native Extractor Decision

Date: 2026-06-19

## Decision

Glory should not add Dioxus-style native framework extractor parameters to the
core `#[glory::server]` ABI yet.

Generated server functions stay adapter-neutral:

- Client-visible parameters are serialized through the server-function wire
  encoding.
- Request metadata is read through `RequestContext`.
- Framework-native extractors remain in Salvo/Axum/Actix custom routes that call
  shared Rust helpers or the generated client-visible server functions.

This preserves the current invariant that Salvo, Axum, and Actix mounts all
dispatch through the same `handle_with_method(path, body)` entry point.

## Why Not Mirror Dioxus Directly

Dioxus' fullstack implementation is Axum-first: the macro can generate an Axum
handler and use `FromRequest` / `FromRequestParts` directly. That is powerful,
but it couples the server-function macro to Axum request/response traits.

Glory currently has three adapter mounts:

- Salvo owns `Request`, `Depot`, and `Response` values inside handler methods.
- Axum uses `FromRequestParts` for parts-only extraction and `FromRequest` for
  body-consuming extraction.
- Actix uses `FromRequest` over `HttpRequest` plus a payload stream.

Those extraction models do not share a common trait. A direct pass-through would
either make `#[server]` adapter-specific or force a lowest-common-denominator
extractor API that would hide important framework behavior.

## Supported Pattern

Use adapter-neutral server functions for client calls:

```rust
#[glory::server]
pub async fn update_title(id: u64, title: String) -> Result<(), glory_serverfn::ServerFnError> {
    let session = glory_serverfn::request_context()
        .and_then(|ctx| ctx.cookie("glory_session"))
        .ok_or_else(|| glory_serverfn::ServerFnError::http(401, "missing session"))?;
    app_update_title(session, id, title).await
}
```

Use native framework routes when the endpoint needs framework state, body
streams, WebSocket upgrades, multipart payload ownership, or framework-specific
auth extractors. Keep business logic in a shared helper:

```rust
async fn app_update_title(
    session: String,
    id: u64,
    title: String,
) -> Result<(), glory_serverfn::ServerFnError> {
    // database / domain logic
    Ok(())
}
```

An Axum route can then use native extractors without changing the
server-function ABI:

```rust
async fn update_title_axum(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Json(input): axum::extract::Json<UpdateTitle>,
) -> Result<axum::http::StatusCode, glory_serverfn::ServerFnError> {
    app_update_title(state.session_id(), input.id, input.title).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
```

## Future Extension Point

If native extractors become necessary inside `#[server]`, implement them as an
explicit adapter extension, not as implicit positional function parameters.

Requirements for that later work:

- Gate every native extractor path by adapter feature (`axum`, `salvo`, or
  `actix`) and keep default features adapter-neutral.
- Keep serialized client arguments separate from server-only extracted values.
- Permit parts-only extractors before the generated request body, but allow at
  most one body-consuming extractor.
- Convert framework rejection types into `ServerFnError` or
  `ServerFnHttpResponse` consistently.
- Add conformance tests proving that enabling one adapter extractor path does
  not change the generated client stub for other adapters.

Until then, `RequestContext` plus custom adapter routes is the supported path.
