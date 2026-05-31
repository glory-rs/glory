//! Actix Web adapter for Glory SSR.

use actix_web::{HttpResponse, http::header::ContentType};
pub use glory_core::web::holders::{HtmlChunk, ServerHolder};

pub fn render_to_string(holder: &ServerHolder) -> String {
    holder.render_string()
}

pub fn into_response(holder: ServerHolder) -> HttpResponse {
    HttpResponse::Ok().content_type(ContentType::html()).body(holder.render_string())
}
