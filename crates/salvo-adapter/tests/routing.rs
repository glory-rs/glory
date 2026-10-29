#![cfg(all(feature = "routing", not(target_arch = "wasm32")))]
use glory_routing::{RouteDef, RouteKey, Routes};
use glory_ui::text;
use http_body_util::BodyExt;
use salvo::{Response, http::StatusCode};
fn routes() -> Routes {
    Routes::builder()
        .root(RouteDef::new(RouteKey::<()>::new("home"), ""))
        .finish()
        .unwrap()
}
fn response(target: &str, head: bool) -> Response {
    let routes = routes();
    let request = glory_salvo::routing::RequestLocation::parse(target)
        .unwrap()
        .resolve(&routes);
    glory_salvo::routing::render_response_with(routes, request, head, |_, handle| {
        text().content(format!(
            "page:{}",
            handle.current().unwrap().location().path()
        ))
    })
    .unwrap()
}
#[tokio::test]
async fn renders_request_status_and_head_without_changing_ssr_markup() {
    let mut home = response("/", false);
    assert_eq!(home.status_code, Some(StatusCode::OK));
    let bytes = home.take_body().collect().await.unwrap().to_bytes();
    assert!(
        String::from_utf8(bytes.to_vec())
            .unwrap()
            .contains("page:/")
    );
    let mut missing = response("/missing", false);
    assert_eq!(missing.status_code, Some(StatusCode::NOT_FOUND));
    assert!(
        String::from_utf8(
            missing
                .take_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec()
        )
        .unwrap()
        .contains("page:/missing")
    );
    let mut head = response("/", true);
    assert!(
        head.take_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .is_empty()
    );
}
#[test]
fn redirects_and_denials_do_not_invoke_the_page_factory() {
    let routes = routes();
    let request = glory_salvo::routing::RequestLocation::parse("/legacy")
        .unwrap()
        .resolve_with(&routes, |location, _| {
            if location.path() == "/legacy" {
                Ok(glory_salvo::routing::Decision::Redirect(
                    glory_routing::Location::parse("/").unwrap(),
                ))
            } else {
                Ok(glory_salvo::routing::Decision::Allow)
            }
        });
    let response =
        glory_salvo::routing::render_response_with(routes.clone(), request, false, |_, _| {
            panic!("redirect cannot render");
            #[allow(unreachable_code)]
            text()
        })
        .unwrap();
    assert_eq!(response.status_code, Some(StatusCode::FOUND));
    assert_eq!(response.headers()["location"], "/");
    let request = glory_salvo::routing::RequestLocation::parse("/")
        .unwrap()
        .resolve_with(&routes, |_, _| {
            Ok(glory_salvo::routing::Decision::Denied(403))
        });
    let response = glory_salvo::routing::render_response_with(routes, request, false, |_, _| {
        panic!("denial cannot render");
        #[allow(unreachable_code)]
        text()
    })
    .unwrap();
    assert_eq!(response.status_code, Some(StatusCode::FORBIDDEN));
}
