//! Salvo adapter for Glory SSR.

pub use glory_core::web::holders::{HtmlChunk, SalvoHandler, ServerHolder};

pub fn render_to_string(holder: &ServerHolder) -> String {
    holder.render_string()
}
