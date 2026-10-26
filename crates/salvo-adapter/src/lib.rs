//! Native Salvo HTTP integration. No HTTP types enter Glory's UI or SSR core.
#![cfg(not(target_arch = "wasm32"))]

pub use glory_ssr as ssr;
use glory_ui::{Scope, Widget};
use salvo::Response;
use salvo::http::{HeaderValue, StatusCode, header};

#[cfg(feature = "stream")]
mod streaming;
#[cfg(feature = "stream")]
pub use streaming::{StreamError, StreamPage, StreamRenderer};

/// Wrap already-rendered HTML, including explicitly assembled resource snapshots.
pub fn html_response(html: impl Into<String>) -> Response {
    let mut response = html_headers();
    response
        .write_body(html.into())
        .expect("fresh response body");
    response
}

/// Render a semantic widget on the calling thread. No runtime survives the call.
pub fn render_response(widget: impl Widget) -> glory_ui::Result<Response> {
    ssr::render(widget).map(html_response)
}

/// Construct reactive state under this request's Owner on the calling thread.
pub fn render_response_with<W: Widget>(
    factory: impl FnOnce(&Scope) -> W,
) -> glory_ui::Result<Response> {
    ssr::render_with(factory).map(html_response)
}

pub fn document_response(
    widget: impl Widget,
    document: &ssr::Document,
) -> glory_ui::Result<Response> {
    ssr::render_document(widget, document).map(html_response)
}

fn html_headers() -> Response {
    let mut response = Response::new();
    response.status_code(StatusCode::OK);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response
}
