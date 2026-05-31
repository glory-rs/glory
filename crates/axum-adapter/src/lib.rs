//! Axum adapter for Glory SSR.

use axum::response::Html;
pub use glory_core::web::holders::{HtmlChunk, ServerHolder};

pub fn render_to_string(holder: &ServerHolder) -> String {
    holder.render_string()
}

pub fn into_response(holder: ServerHolder) -> Html<String> {
    Html(holder.render_string())
}
