# Server Function Adapter Recipes

These recipes cover custom routes that sit next to generated `#[server]`
endpoints. Generated server functions stay JSON/form request-response handlers;
use custom routes for chunk flushing, SSE, uploads, and login/logout redirects.

## Streaming And SSE

Build adapter-agnostic responses with `StreamingResponse`:

```rust
fn todo_events() -> glory_serverfn::StreamingResponse {
    glory_serverfn::StreamingResponse::sse([
        glory_serverfn::SseEvent::named("todo", r#"{"id":1}"#).id("1"),
    ])
    .with_header("cache-control", "no-cache")
}
```

Salvo:

```rust
fn events_response() -> Result<salvo::prelude::Response, glory_serverfn::ServerFnError> {
    glory_serverfn::salvo_mount::streaming_response(todo_events())
}
```

When a Salvo handler already receives `&mut Response`, write into it directly:

```rust
#[salvo::handler]
async fn events(res: &mut salvo::prelude::Response) {
    glory_serverfn::salvo_mount::write_streaming_response(res, todo_events()).unwrap();
}
```

Axum:

```rust
async fn events() -> Result<axum::response::Response, glory_serverfn::ServerFnError> {
    glory_serverfn::axum_mount::streaming_response(todo_events())
}
```

Actix:

```rust
async fn events() -> Result<actix_web::HttpResponse, glory_serverfn::ServerFnError> {
    glory_serverfn::actix_mount::streaming_response(todo_events())
}
```

## WebSocket Envelopes

Keep the framework-specific socket accept loop in the app crate and use Glory's
typed envelope helpers for the wire payload:

```rust
#[derive(serde::Serialize, serde::Deserialize)]
struct Notice {
    title: String,
}

let outbound = glory_serverfn::TransportMessage::data(Notice {
    title: "created".into(),
});
let frame = glory_serverfn::WebSocketFrame::text_json(&outbound)?;
let decoded: glory_serverfn::TransportMessage<Notice> = frame.decode_json()?;
```

## Multipart Uploads

Use `decode_multipart` directly when the framework route owns the request body.

Salvo:

```rust
#[salvo::handler]
async fn upload(req: &mut salvo::prelude::Request, res: &mut salvo::prelude::Response) {
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let body = req.payload().await.unwrap();
    let form = glory_serverfn::decode_multipart(
        content_type,
        &body,
        glory_serverfn::MultipartLimits::default(),
    )
    .unwrap();
    res.render(format!("uploaded: {}", form.file("avatar").is_some()));
}
```

Axum:

```rust
async fn upload(
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<String, glory_serverfn::ServerFnError> {
    let content_type = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let form = glory_serverfn::decode_multipart(
        content_type,
        &body,
        glory_serverfn::MultipartLimits::default(),
    )?;
    Ok(form.text("title")?.unwrap_or_default())
}
```

Actix:

```rust
async fn upload(
    req: actix_web::HttpRequest,
    body: actix_web::web::Bytes,
) -> Result<actix_web::HttpResponse, glory_serverfn::ServerFnError> {
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let form = glory_serverfn::decode_multipart(
        content_type,
        &body,
        glory_serverfn::MultipartLimits::default(),
    )?;
    Ok(actix_web::HttpResponse::Ok().body(form.text("title")?.unwrap_or_default()))
}
```

## Login And Logout

Server functions can read cookies through `RequestContext`:

```rust
#[glory::server]
pub async fn session_label() -> Result<String, glory_serverfn::ServerFnError> {
    Ok(glory_serverfn::request_context()
        .and_then(|ctx| ctx.cookie("glory_session"))
        .unwrap_or_else(|| "anonymous".to_owned()))
}
```

For form-action login/logout routes, return redirects with `Set-Cookie`
headers:

```rust
fn login_redirect(user_id: &str) -> Result<(), glory_serverfn::ServerFnError> {
    let cookie = glory_serverfn::set_cookie_header(
        "glory_session",
        user_id,
        glory_serverfn::CookieOptions::default().secure(true),
    )?;
    Err(glory_serverfn::ServerFnError::redirect("/").with_header("set-cookie", cookie))
}

fn logout_redirect() -> Result<(), glory_serverfn::ServerFnError> {
    let cookie = glory_serverfn::clear_cookie_header("glory_session", "/")?;
    Err(glory_serverfn::ServerFnError::redirect("/login").with_header("set-cookie", cookie))
}
```

The adapter mounts already forward `ServerFnHttpError` headers, so these
redirects work through Salvo, Axum, and Actix server-function mounts.
