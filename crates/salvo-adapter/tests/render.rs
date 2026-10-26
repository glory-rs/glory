#![cfg(not(target_arch = "wasm32"))]

use std::cell::Cell;
use std::rc::Rc;

use glory::prelude::*;
use glory_salvo::{document_response, html_response, render_response, render_response_with, ssr};
use http_body_util::BodyExt;
use salvo::http::StatusCode;
use salvo::prelude::*;

async fn body(mut response: Response) -> String {
    assert_eq!(response.status_code, Some(StatusCode::OK));
    assert_eq!(
        response.headers()["content-type"],
        "text/html; charset=utf-8"
    );
    String::from_utf8(
        response
            .take_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap()
}

#[tokio::test]
async fn semantic_render_escapes_and_canonicalizes_controls() {
    let response = render_response(view().class("greeting").fill((
        text().content("hello <Salvo> & Glory"),
        input().multiline(true).value("\r\nline\0"),
    )))
    .unwrap();
    let html = body(response).await;
    assert!(html.contains("class=\"greeting\""));
    assert!(html.contains("hello &lt;Salvo&gt; &amp; Glory"));
    assert!(html.contains(">\n\nline�</textarea>"));
}

#[tokio::test]
async fn reactive_factory_is_request_owned_and_disposes_before_returning() {
    let cleaned = Rc::new(Cell::new(false));
    let response = render_response_with(|scope| {
        let cleaned = cleaned.clone();
        scope.owner().on_cleanup(move || cleaned.set(true)).unwrap();
        let value = scope.owner().cage(String::from("request state"));
        text().content(value)
    })
    .unwrap();
    assert!(cleaned.get());
    assert!(body(response).await.contains("request state"));
}

#[tokio::test]
async fn complete_document_uses_shared_prefix_and_escapes_metadata() {
    let document = ssr::Document {
        title: "A < B & C".into(),
        lang: Some("en\"".into()),
        stylesheets: vec!["/main.css?a=1&b=2".into(), "/main.css?a=1&b=2".into()],
        nonce: Some("nonce\"<&".into()),
        ..Default::default()
    };
    let html = body(document_response(text().content("body"), &document).unwrap()).await;
    assert!(html.starts_with(&document.opening_html().unwrap()));
    assert!(html.contains("<title>A &lt; B &amp; C</title>"));
    assert!(html.contains("lang=\"en&quot;\""));
    assert!(html.contains("nonce=\"nonce&quot;&lt;&amp;\""));
    assert_eq!(html.matches("/main.css?a=1&amp;b=2").count(), 1);
    assert!(html.ends_with("</body></html>"));
}

#[test]
fn invalid_widget_returns_typed_error_without_a_partial_response() {
    assert!(render_response(input().value("two\nlines")).is_err());
}

#[handler]
async fn hello(res: &mut Response) {
    *res =
        render_response_with(|scope| text().content(scope.owner().cage("Salvo route".to_owned())))
            .unwrap();
}

#[tokio::test]
async fn runs_in_a_real_salvo_handler() {
    let response = salvo::test::TestClient::get("http://localhost/")
        .send(Router::new().get(hello))
        .await;
    assert!(body(response).await.contains("Salvo route"));
}

#[tokio::test]
async fn snapshot_can_be_wrapped_without_rewriting_hydration_markers() {
    let snapshot = ssr::snapshot(text().content("hydrate me")).unwrap();
    assert_eq!(
        body(html_response(snapshot.html.clone())).await,
        snapshot.html
    );
}

#[cfg(feature = "resources")]
#[tokio::test]
async fn seeded_resource_snapshot_exports_before_request_cleanup() {
    let mut seed = glory::resources::Snapshot::default();
    seed.seed(
        "profile",
        "v1",
        &7_u64,
        Ok::<_, &String>(&"request <A>".to_owned()),
    )
    .unwrap();
    let resources = Resources::restore(seed).unwrap();
    let snapshot = ssr::snapshot_resources_with(&resources, |scope| {
        let value = resources
            .bind(
                scope.owner(),
                "profile",
                "v1",
                || 7_u64,
                |_| {
                    std::future::poll_fn(
                        |_| -> std::task::Poll<std::result::Result<String, String>> {
                            panic!("matching seed must suppress the loader")
                        },
                    )
                },
            )
            .unwrap();
        text().content(value.map(|state| match state {
            ResourceState::Ready { value, .. } => value.to_string(),
            _ => "pending".into(),
        }))
    })
    .unwrap();
    assert!(resources.is_empty());
    let script = ssr::resource_script("data", &snapshot.data, None).unwrap();
    let html = body(html_response(format!("{}{}", snapshot.view.html, script))).await;
    assert!(html.contains("request &lt;A&gt;"));
    assert!(html.contains("request \\u003cA\\u003e"));
}

#[cfg(feature = "css")]
#[tokio::test]
async fn stylesheet_feature_is_forwarded_to_document_output() {
    let sheet = glory::html::Sheet::parse(".example { opacity: 0.5; }").unwrap();
    let document = ssr::Document {
        sheets: vec![sheet],
        ..Default::default()
    };
    let html =
        body(document_response(text().class("example").content("styled"), &document).unwrap())
            .await;
    assert!(html.contains("data-gly-sheet"));
    assert!(html.contains("opacity:0.5"));
}
