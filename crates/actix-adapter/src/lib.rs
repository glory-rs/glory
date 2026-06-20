//! Actix Web adapter for Glory SSR.

use std::convert::Infallible;

use actix_web::{HttpResponse, http::header::ContentType};
use bytes::Bytes;
pub use glory_core::web::holders::{HtmlChunk, ServerHolder};

pub fn render_to_string(holder: &ServerHolder) -> String {
    holder.render_string()
}

pub fn into_response(holder: ServerHolder) -> HttpResponse {
    HttpResponse::Ok().content_type(ContentType::html()).body(holder.render_string())
}

/// Streams the holder's [`HtmlChunk`] sequence as an Actix chunked body via
/// [`HttpResponse::streaming`], with a `text/html` content type.
///
/// `ServerHolder` is `!Send` (the reactive runtime holds `Rc`s) while Actix's
/// `streaming` body requires a `Stream + 'static`. We drive the holder's render
/// to completion on *this* thread up front — collapsing the whole [`HtmlChunk`]
/// sequence into an owned `Vec<Bytes>` — and only then feed it to
/// [`futures::stream::iter`]. The resulting stream owns plain `Bytes`, so it
/// satisfies the body API while every `!Send` step stays on the current thread.
///
/// This wires up the streaming-body transport (the prerequisite pipeline for
/// first-byte flush); true incremental parse-and-flush over the wire is bounded
/// by the `!Send` runtime and is tracked separately as FS4.
pub fn into_streaming_response(holder: ServerHolder) -> HttpResponse {
    let chunks: Vec<Bytes> = futures::executor::block_on(async {
        use futures::StreamExt;
        holder.render_stream().map(|chunk| Bytes::from(chunk.into_string())).collect().await
    });
    let stream = futures::stream::iter(chunks.into_iter().map(Result::<Bytes, Infallible>::Ok));
    HttpResponse::Ok().content_type(ContentType::html()).streaming(stream)
}
