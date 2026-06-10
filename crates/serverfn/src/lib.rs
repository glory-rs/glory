//! Server functions runtime for Glory.
//!
//! A *server function* is an `async fn` annotated with
//! `#[glory_macros::server]`: its body compiles into server builds only,
//! while wasm builds get a stub that POSTs the arguments to
//! `/__glory/fn/<name>` and deserializes the response. This crate is the
//! runtime both sides share:
//!
//! - [`ServerFnEntry`] / [`handle`] — the inventory-backed registry the
//!   macro registers into and adapter mounts dispatch through.
//! - [`call_remote`] — the client leg (wasm `fetch`, or `reqwest` under the
//!   `reqwest-client` feature for non-wasm clients such as desktop apps).
//! - [`salvo_mount`] / [`axum_mount`] / [`actix_mount`] — one-line router
//!   integration per supported web framework.
//!
//! # Wire format
//!
//! Arguments serialize as a JSON tuple (`(a, b, c)`), responses as plain
//! JSON of the `Ok` value. Errors map to HTTP 500 with a JSON-encoded
//! [`ServerFnError`] body, which the client leg decodes back into the same
//! enum — so `?` propagation works symmetrically on both sides.

use serde::Serialize;
use serde::de::DeserializeOwned;

#[cfg(not(target_arch = "wasm32"))]
pub use inventory;

/// Errors crossing the server-function boundary. Serializable so the
/// server leg can transport the failure to the client leg verbatim.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum ServerFnError {
    /// Transport-level failure (network, non-JSON response, ...).
    #[error("server fn request failed: {0}")]
    Request(String),
    #[error("server fn argument serialization failed: {0}")]
    Serialization(String),
    #[error("server fn result deserialization failed: {0}")]
    Deserialization(String),
    /// No function registered under the requested path.
    #[error("no server fn registered at {0}")]
    NotFound(String),
    /// The function body itself failed. `String` keeps the error
    /// serializable; convert domain errors with `.to_string()` / `From`.
    #[error("server fn failed: {0}")]
    ServerError(String),
}

impl From<String> for ServerFnError {
    fn from(value: String) -> Self {
        Self::ServerError(value)
    }
}
impl From<&str> for ServerFnError {
    fn from(value: &str) -> Self {
        Self::ServerError(value.to_owned())
    }
}

/// URL prefix every generated endpoint lives under.
pub const PREFIX: &str = "/__glory/fn";

pub fn decode_args<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ServerFnError> {
    serde_json::from_slice(bytes).map_err(|err| ServerFnError::Deserialization(err.to_string()))
}

pub fn encode_args<T: Serialize>(value: &T) -> Result<Vec<u8>, ServerFnError> {
    serde_json::to_vec(value).map_err(|err| ServerFnError::Serialization(err.to_string()))
}

pub fn encode_ok<T: Serialize>(value: &T) -> Result<Vec<u8>, ServerFnError> {
    serde_json::to_vec(value).map_err(|err| ServerFnError::Serialization(err.to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
pub type BoxedServerFnFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, ServerFnError>> + Send>>;

/// One registered server function. The `#[server]` macro submits these
/// into the global [`inventory`] registry at link time.
#[cfg(not(target_arch = "wasm32"))]
pub struct ServerFnEntry {
    /// Full URL path, e.g. `/__glory/fn/list_todos`.
    pub path: &'static str,
    pub handler: fn(Vec<u8>) -> BoxedServerFnFuture,
}

#[cfg(not(target_arch = "wasm32"))]
inventory::collect!(ServerFnEntry);

/// All registered endpoint paths (diagnostics / route listing).
#[cfg(not(target_arch = "wasm32"))]
pub fn registered_paths() -> Vec<&'static str> {
    inventory::iter::<ServerFnEntry>.into_iter().map(|entry| entry.path).collect()
}

/// Dispatches a raw request body to the function registered at `path`.
/// This is the single entry point every adapter mount goes through.
#[cfg(not(target_arch = "wasm32"))]
pub async fn handle(path: &str, body: Vec<u8>) -> Result<Vec<u8>, ServerFnError> {
    for entry in inventory::iter::<ServerFnEntry> {
        if entry.path == path {
            return (entry.handler)(body).await;
        }
    }
    Err(ServerFnError::NotFound(path.to_owned()))
}

// ---------------------------------------------------------------------------
// Request context (server side)
// ---------------------------------------------------------------------------

/// Snapshot of the HTTP request a server function is handling. Populated
/// by the adapter mounts before dispatch; absent when a server function is
/// called directly (SSR rendering, tests).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Default)]
pub struct RequestContext {
    pub method: String,
    pub uri: String,
    /// Header pairs, names lowercased.
    pub headers: Vec<(String, String)>,
}

#[cfg(not(target_arch = "wasm32"))]
impl RequestContext {
    /// First value of `name` (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers.iter().find(|(key, _)| *key == name).map(|(_, value)| value.as_str())
    }
}

#[cfg(not(target_arch = "wasm32"))]
tokio::task_local! {
    static REQUEST_CONTEXT: RequestContext;
}

/// Runs `future` with `context` installed as the task-local request
/// context. Adapter mounts wrap [`handle`] in this; custom integrations
/// should do the same.
#[cfg(not(target_arch = "wasm32"))]
pub async fn with_request_context<F: std::future::Future>(context: RequestContext, future: F) -> F::Output {
    REQUEST_CONTEXT.scope(context, future).await
}

/// The current request's context, when running under an adapter mount.
/// `None` for direct calls (SSR rendering, tests) — treat that as "no
/// HTTP request", not an error.
#[cfg(not(target_arch = "wasm32"))]
pub fn request_context() -> Option<RequestContext> {
    REQUEST_CONTEXT.try_with(|context| context.clone()).ok()
}

// ---------------------------------------------------------------------------
// Client leg
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
static SERVER_URL: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());

/// Base URL prepended to endpoint paths by non-wasm clients (desktop apps
/// calling a remote server). Wasm clients are same-origin and ignore this
/// unless set.
#[cfg(not(target_arch = "wasm32"))]
pub fn set_server_url(url: impl Into<String>) {
    let mut value = url.into();
    while value.ends_with('/') {
        value.pop();
    }
    *SERVER_URL.write().expect("server url lock") = value;
}

#[cfg(not(target_arch = "wasm32"))]
pub fn server_url() -> String {
    SERVER_URL.read().expect("server url lock").clone()
}

/// Client leg of a server function call: POST the JSON-tuple `args` to
/// `path` and decode the JSON response. Generated stubs call this; user
/// code normally never does.
pub async fn call_remote<Args, Out>(path: &str, args: &Args) -> Result<Out, ServerFnError>
where
    Args: Serialize,
    Out: DeserializeOwned,
{
    let body = encode_args(args)?;

    #[cfg(target_arch = "wasm32")]
    {
        call_remote_wasm(path, body)
            .await
            .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|err| ServerFnError::Deserialization(err.to_string())))
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "reqwest-client"))]
    {
        let url = format!("{}{}", server_url(), path);
        let response = reqwest::Client::new()
            .post(&url)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|err| ServerFnError::Request(err.to_string()))?;
        let status = response.status();
        let bytes = response.bytes().await.map_err(|err| ServerFnError::Request(err.to_string()))?;
        if status.is_success() {
            serde_json::from_slice(&bytes).map_err(|err| ServerFnError::Deserialization(err.to_string()))
        } else {
            Err(serde_json::from_slice::<ServerFnError>(&bytes).unwrap_or_else(|_| ServerFnError::Request(format!("HTTP {status}"))))
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "reqwest-client")))]
    {
        let _ = (path, body);
        Err(ServerFnError::Request(
            "no HTTP client available: enable the `reqwest-client` feature of glory-serverfn for non-wasm clients".to_owned(),
        ))
    }
}

#[cfg(target_arch = "wasm32")]
async fn call_remote_wasm(path: &str, body: Vec<u8>) -> Result<Vec<u8>, ServerFnError> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let request_err = |err: wasm_bindgen::JsValue| ServerFnError::Request(format!("{err:?}"));

    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    let body_value = js_sys::Uint8Array::from(body.as_slice());
    init.set_body(&body_value);
    let request = web_sys::Request::new_with_str_and_init(path, &init).map_err(request_err)?;
    request.headers().set("content-type", "application/json").map_err(request_err)?;

    let window = web_sys::window().ok_or_else(|| ServerFnError::Request("no window".to_owned()))?;
    let response: web_sys::Response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(request_err)?
        .dyn_into()
        .map_err(request_err)?;

    let buffer = JsFuture::from(response.array_buffer().map_err(request_err)?).await.map_err(request_err)?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    if response.ok() {
        Ok(bytes)
    } else {
        Err(serde_json::from_slice::<ServerFnError>(&bytes).unwrap_or_else(|_| ServerFnError::Request(format!("HTTP {}", response.status()))))
    }
}

// ---------------------------------------------------------------------------
// Adapter mounts
// ---------------------------------------------------------------------------

/// Salvo integration: `router.push(glory_serverfn::salvo_mount::router())`.
#[cfg(all(feature = "salvo", not(target_arch = "wasm32")))]
pub mod salvo_mount {
    use salvo::http::StatusCode;
    use salvo::prelude::*;

    #[handler]
    async fn server_fn_handler(req: &mut Request, res: &mut Response) {
        let path = req.uri().path().to_owned();
        let context = crate::RequestContext {
            method: req.method().to_string(),
            uri: req.uri().to_string(),
            headers: req
                .headers()
                .iter()
                .map(|(name, value)| (name.as_str().to_ascii_lowercase(), value.to_str().unwrap_or_default().to_owned()))
                .collect(),
        };
        let body = match req.payload().await {
            Ok(bytes) => bytes.to_vec(),
            Err(err) => {
                res.status_code(StatusCode::BAD_REQUEST);
                res.render(format!("invalid body: {err}"));
                return;
            }
        };
        match crate::with_request_context(context, crate::handle(&path, body)).await {
            Ok(bytes) => {
                res.add_header("content-type", "application/json", true).ok();
                let _ = res.write_body(bytes);
            }
            Err(err) => {
                let status = if matches!(err, crate::ServerFnError::NotFound(_)) {
                    StatusCode::NOT_FOUND
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                };
                res.status_code(status);
                res.add_header("content-type", "application/json", true).ok();
                let _ = res.write_body(serde_json::to_vec(&err).expect("ServerFnError serializes"));
            }
        }
    }

    /// Router serving every registered server function under
    /// [`crate::PREFIX`]. Push it into your app router as-is.
    pub fn router() -> Router {
        Router::with_path("__glory/fn/{**rest}").post(server_fn_handler)
    }
}

/// Axum integration: `app.merge(glory_serverfn::axum_mount::router())`.
#[cfg(all(feature = "axum", not(target_arch = "wasm32")))]
pub mod axum_mount {
    use axum::http::StatusCode;
    use axum::response::{IntoResponse, Response};

    async fn server_fn_handler(request: axum::extract::Request) -> Response {
        let path = request.uri().path().to_owned();
        let context = crate::RequestContext {
            method: request.method().to_string(),
            uri: request.uri().to_string(),
            headers: request
                .headers()
                .iter()
                .map(|(name, value)| (name.as_str().to_ascii_lowercase(), value.to_str().unwrap_or_default().to_owned()))
                .collect(),
        };
        let body = match axum::body::to_bytes(request.into_body(), 16 * 1024 * 1024).await {
            Ok(bytes) => bytes.to_vec(),
            Err(err) => return (StatusCode::BAD_REQUEST, format!("invalid body: {err}")).into_response(),
        };
        match crate::with_request_context(context, crate::handle(&path, body)).await {
            Ok(bytes) => ([("content-type", "application/json")], bytes).into_response(),
            Err(err) => {
                let status = if matches!(err, crate::ServerFnError::NotFound(_)) {
                    StatusCode::NOT_FOUND
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                };
                (
                    status,
                    [("content-type", "application/json")],
                    serde_json::to_vec(&err).expect("ServerFnError serializes"),
                )
                    .into_response()
            }
        }
    }

    /// Router serving every registered server function under
    /// [`crate::PREFIX`]. Merge it into your app router.
    pub fn router<S: Clone + Send + Sync + 'static>() -> axum::Router<S> {
        axum::Router::new().route(&format!("{}/{{*rest}}", crate::PREFIX), axum::routing::post(server_fn_handler))
    }
}

/// Actix integration:
/// `App::new().configure(glory_serverfn::actix_mount::configure)`.
#[cfg(all(feature = "actix", not(target_arch = "wasm32")))]
pub mod actix_mount {
    use actix_web::{HttpRequest, HttpResponse, web};

    async fn server_fn_handler(request: HttpRequest, body: web::Bytes) -> HttpResponse {
        let path = request.uri().path().to_owned();
        let context = crate::RequestContext {
            method: request.method().to_string(),
            uri: request.uri().to_string(),
            headers: request
                .headers()
                .iter()
                .map(|(name, value)| (name.as_str().to_ascii_lowercase(), value.to_str().unwrap_or_default().to_owned()))
                .collect(),
        };
        match crate::with_request_context(context, crate::handle(&path, body.to_vec())).await {
            Ok(bytes) => HttpResponse::Ok().content_type("application/json").body(bytes),
            Err(err) => {
                let mut builder = if matches!(err, crate::ServerFnError::NotFound(_)) {
                    HttpResponse::NotFound()
                } else {
                    HttpResponse::InternalServerError()
                };
                builder
                    .content_type("application/json")
                    .body(serde_json::to_vec(&err).expect("ServerFnError serializes"))
            }
        }
    }

    /// Registers the server-function dispatch route under [`crate::PREFIX`].
    pub fn configure(config: &mut web::ServiceConfig) {
        config.route(&format!("{}/{{rest:.*}}", crate::PREFIX), web::post().to(server_fn_handler));
    }
}
