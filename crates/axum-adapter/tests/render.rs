//! Behaviour coverage for the Axum SSR adapter: a mounted widget renders to
//! HTML and `into_response` wraps it as `Html<String>`.

use axum::http::StatusCode;
use glory_axum::{ServerHolder, into_response, into_streaming_response, render_to_string};
use glory_core::web::widgets::div;
use glory_core::{GloryConfig, Holder, Scope, Widget};

#[derive(Debug)]
struct Hello;

impl Widget for Hello {
    fn build(&mut self, ctx: &mut Scope) {
        div().class("greeting").text("hello-axum").show_in(ctx);
    }
}

#[test]
fn render_to_string_emits_widget_html() {
    let holder = ServerHolder::new(GloryConfig::default(), "/").mount(Hello);
    let html = render_to_string(&holder);
    assert!(html.contains("hello-axum"), "{html}");
    assert!(html.contains("greeting"), "{html}");
}

#[test]
fn into_response_wraps_rendered_html() {
    let holder = ServerHolder::new(GloryConfig::default(), "/").mount(Hello);
    let html = into_response(holder);
    assert!(html.0.contains("hello-axum"), "{}", html.0);
}

#[test]
fn into_streaming_response_sets_ok_status_and_html_content_type() {
    let holder = ServerHolder::new(GloryConfig::default(), "/").mount(Hello);
    let response = into_streaming_response(holder);
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(content_type.contains("text/html"), "content-type was {content_type:?}");
}

#[test]
fn into_streaming_response_body_contains_widget_html() {
    let holder = ServerHolder::new(GloryConfig::default(), "/").mount(Hello);
    let response = into_streaming_response(holder);
    let body = futures::executor::block_on(async { axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap() });
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("hello-axum"), "{body}");
}
