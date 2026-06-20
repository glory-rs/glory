//! Behaviour coverage for the Actix SSR adapter: a mounted widget renders to
//! HTML and `into_response` produces a 200 text/html response.

use actix_web::http::StatusCode;
use glory_actix::{ServerHolder, into_response, into_streaming_response, render_to_string};
use glory_core::web::widgets::div;
use glory_core::{GloryConfig, Holder, Scope, Widget};

#[derive(Debug)]
struct Hello;

impl Widget for Hello {
    fn build(&mut self, ctx: &mut Scope) {
        div().class("greeting").text("hello-actix").show_in(ctx);
    }
}

#[test]
fn render_to_string_emits_widget_html() {
    let holder = ServerHolder::new(GloryConfig::default(), "/").mount(Hello);
    let html = render_to_string(&holder);
    assert!(html.contains("hello-actix"), "{html}");
    assert!(html.contains("greeting"), "{html}");
}

#[test]
fn into_response_sets_ok_status_and_html_content_type() {
    let holder = ServerHolder::new(GloryConfig::default(), "/").mount(Hello);
    let response = into_response(holder);
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(content_type.contains("text/html"), "content-type was {content_type:?}");
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
    let body = futures::executor::block_on(async { actix_web::body::to_bytes(response.into_body()).await.unwrap() });
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("hello-actix"), "{body}");
}
