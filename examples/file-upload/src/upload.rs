//! Server-side multipart upload handling, built on the serverfn
//! `MultipartForm` / `MultipartPart` primitives.
//!
//! A plain `<form method="post" enctype="multipart/form-data">` posts here.
//! We parse the body with [`glory::serverfn::decode_multipart`], read the text
//! field plus the uploaded file part, and render a small confirmation page.

#![cfg(feature = "web-ssr")]

use glory::serverfn::{MultipartForm, MultipartLimits, ServerFnError};
use salvo::prelude::*;

/// One accepted upload, summarised for display.
#[derive(Clone, Debug)]
struct UploadSummary {
    title: String,
    filename: String,
    content_type: String,
    byte_len: usize,
    preview: String,
}

fn summarise(form: &MultipartForm) -> Result<UploadSummary, ServerFnError> {
    let title = form.text("title")?.unwrap_or_default();
    let file = form
        .file("file")
        .ok_or_else(|| ServerFnError::bad_request("missing `file` part"))?;

    let preview = String::from_utf8_lossy(&file.bytes).chars().take(120).collect::<String>();

    Ok(UploadSummary {
        title: if title.trim().is_empty() { "(untitled)".to_owned() } else { title },
        filename: file.filename.clone().unwrap_or_else(|| "(unnamed)".to_owned()),
        content_type: file.content_type.clone().unwrap_or_else(|| "application/octet-stream".to_owned()),
        byte_len: file.bytes.len(),
        preview,
    })
}

#[handler]
pub async fn upload_handler(req: &mut Request, res: &mut Response) {
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();

    let body = match req.payload().await {
        Ok(bytes) => bytes.to_vec(),
        Err(err) => {
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Text::Html(format!("<p>could not read body: {err}</p>")));
            return;
        }
    };

    let limits = MultipartLimits {
        max_file_bytes: 4 * 1024 * 1024,
        ..Default::default()
    };

    match glory::serverfn::decode_multipart(&content_type, &body, limits).and_then(|form| summarise(&form)) {
        Ok(summary) => {
            res.render(Text::Html(render_result_page(&summary)));
        }
        Err(err) => {
            res.status_code(StatusCode::from_u16(err.status_code()).unwrap_or(StatusCode::BAD_REQUEST));
            res.render(Text::Html(format!(
                "<main style=\"font-family:system-ui;max-width:40rem;margin:2rem auto\">\
                 <h1>Upload failed</h1><p>{err}</p><p><a href=\"/\">Back</a></p></main>"
            )));
        }
    }
}

fn render_result_page(summary: &UploadSummary) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Upload received</title></head>\
         <body style=\"font-family:system-ui;max-width:40rem;margin:2rem auto\">\
         <h1>Upload received</h1>\
         <ul>\
         <li><b>title:</b> {title}</li>\
         <li><b>filename:</b> {filename}</li>\
         <li><b>content-type:</b> {content_type}</li>\
         <li><b>bytes:</b> {byte_len}</li>\
         </ul>\
         <h2>Preview</h2><pre>{preview}</pre>\
         <p><a href=\"/\">Upload another</a></p>\
         </body></html>",
        title = html_escape(&summary.title),
        filename = html_escape(&summary.filename),
        content_type = html_escape(&summary.content_type),
        byte_len = summary.byte_len,
        preview = html_escape(&summary.preview),
    )
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
