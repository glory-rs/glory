//! Salvo adapter for Glory SSR.

pub use glory_core::web::holders::{HtmlChunk, SalvoHandler, ServerHolder};
use salvo::prelude::{Response, StatusCode};

pub fn render_to_string(holder: &ServerHolder) -> String {
    holder.render_string()
}

pub fn into_response(holder: ServerHolder) -> Response {
    let mut response = Response::new();
    response.status_code(StatusCode::OK);
    let _ = response.add_header("content-type", "text/html", true);
    let _ = response.write_body(holder.render_string());
    response
}
