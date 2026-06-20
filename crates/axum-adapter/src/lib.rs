//! Axum adapter for Glory SSR.

use std::convert::Infallible;

use axum::body::Body;
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use bytes::Bytes;
pub use glory_core::web::holders::{HtmlChunk, ServerHolder};

pub fn render_to_string(holder: &ServerHolder) -> String {
    holder.render_string()
}

pub fn into_response(holder: ServerHolder) -> Html<String> {
    Html(holder.render_string())
}

/// Streams the holder's [`HtmlChunk`] sequence as an Axum chunked body
/// (`Transfer-Encoding: chunked`) with a `text/html` content type.
///
/// `ServerHolder` itself is `!Send` (the reactive runtime holds `Rc`s) and
/// [`Body::from_stream`] requires a `Stream + Send`. We sidestep that by driving
/// the holder's render to completion on *this* thread up front — collapsing the
/// whole [`HtmlChunk`] sequence into an owned `Vec<Bytes>` — and only then
/// handing it to [`futures::stream::iter`]. The resulting stream is `Send`
/// because `Bytes` is `Send`, so it satisfies the body API while every `!Send`
/// step stays on the current thread.
///
/// This wires up the streaming-body transport (the prerequisite pipeline for
/// first-byte flush); true incremental parse-and-flush over the wire is bounded
/// by the `!Send` runtime and is tracked separately as FS4.
pub fn into_streaming_response(holder: ServerHolder) -> Response {
    let chunks: Vec<Bytes> = futures::executor::block_on(async {
        use futures::StreamExt;
        holder.render_stream().map(|chunk| Bytes::from(chunk.into_string())).collect().await
    });
    let stream = futures::stream::iter(chunks.into_iter().map(Result::<Bytes, Infallible>::Ok));
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        Body::from_stream(stream),
    )
        .into_response()
}
