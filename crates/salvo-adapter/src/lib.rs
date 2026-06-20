//! Salvo adapter for Glory SSR.

use std::convert::Infallible;

use futures::StreamExt;
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

/// Streams the holder's [`HtmlChunk`] sequence as a Salvo chunked stream body
/// (via [`Response::stream`]) with a `text/html` content type. This mirrors the
/// `Scribe for ServerHolder` impl in core under an `into_*` name aligned with
/// the axum/actix adapters.
///
/// `ServerHolder` is `!Send`, but `render_stream()` materializes the whole
/// [`HtmlChunk`] sequence eagerly (the `!Send` render work runs on this thread
/// before the stream is constructed); the stream then only yields already-owned
/// `String` chunks, so it is safe to hand to Salvo's stream body.
///
/// This wires up the streaming-body transport (the prerequisite pipeline for
/// first-byte flush); true incremental parse-and-flush over the wire is bounded
/// by the `!Send` runtime and is tracked separately as FS4.
pub fn into_streaming_response(holder: ServerHolder) -> Response {
    let mut response = Response::new();
    response.status_code(StatusCode::OK);
    let _ = response.add_header("content-type", "text/html", true);
    response.stream(holder.render_stream().map(|chunk| Result::<_, Infallible>::Ok(chunk.into_string())));
    response
}
