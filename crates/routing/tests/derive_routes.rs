//! RT7: integration coverage for `#[derive(Routable)]` driving real navigation.
//!
//! The derive macro resolves its helper paths via `proc-macro-crate`. From an
//! integration test of `glory-routing`, that resolves to the external crate
//! path, so this exercises the same code path a downstream app would hit.
//!
//! Navigation is driven through the public `MemoryAviator` / `AviatorExt`
//! surface, and (under `web-ssr`) `ServerAviator::with_redirects` is exercised
//! for declarative redirect rewriting.

use std::cell::RefCell;
use std::rc::Rc;

use glory_core::{Scope, Truck, Widget};
use glory_routing::aviators::MemoryAviator;
use glory_routing::{Aviator, AviatorExt, Handler, Locator, Routable, Router, TruckExt};

#[derive(Debug, Clone, PartialEq, Eq, glory_macros::Routable)]
enum AppRoute {
    #[route("/")]
    Home,
    #[route("/users/<id>")]
    #[redirect("/u/<id>")]
    User { id: u64 },
    #[route("/search?q&page&tag")]
    Search { q: String, page: Option<u32>, tag: Vec<String> },
    #[route("/files/<**rest>")]
    Files { rest: Vec<String> },
    #[not_found]
    NotFound { raw_url: String },
}

#[derive(Debug)]
struct UserPage;
impl Widget for UserPage {
    fn build(&mut self, _ctx: &mut Scope) {}
}

#[derive(Debug)]
struct FilesPage;
impl Widget for FilesPage {
    fn build(&mut self, _ctx: &mut Scope) {}
}

#[derive(Debug)]
struct NotFoundPage;
impl Widget for NotFoundPage {
    fn build(&mut self, _ctx: &mut Scope) {}
}

fn aviator(router: Router) -> MemoryAviator {
    let catcher: Rc<dyn Handler> = Rc::new(|truck: Rc<RefCell<Truck>>| truck.insert_stuff("page@404", NotFoundPage));
    let av = MemoryAviator::new(router, move |truck: Rc<RefCell<Truck>>| catcher.handle(truck));
    av.truck.borrow_mut().inject(Locator::new());
    av
}

#[test]
fn derive_to_url_round_trips_through_aviator_navigation() {
    let router = Router::new()
        .push(Router::with_path("users/<id>").outlet("page@user", || UserPage))
        .push(Router::with_path("files/<**rest>").outlet("page@files", || FilesPage));

    let av = aviator(router);

    // Drive navigation with a typed route via AviatorExt::goto_route.
    av.goto_route(&AppRoute::User { id: 7 }).unwrap();
    assert_eq!(av.current().as_deref(), Some("/users/7"));
    assert!(av.truck.contains_stuff_key("page", "user"));

    av.goto_route(&AppRoute::Files {
        rest: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
    })
    .unwrap();
    assert_eq!(av.current().as_deref(), Some("/files/a/b/c"));
    assert!(av.truck.contains_stuff_key("page", "files"));
}

#[test]
fn derive_resolve_url_applies_redirect_then_not_found() {
    // Redirect: legacy `/u/<id>` resolves to the `User` variant.
    assert_eq!(AppRoute::resolve_url("/u/42"), Some(AppRoute::User { id: 42 }));
    // Concrete route still parses directly.
    assert_eq!(AppRoute::resolve_url("/users/9"), Some(AppRoute::User { id: 9 }));
    assert_eq!(AppRoute::resolve_url("/"), Some(AppRoute::Home));
    // Unmatched URL falls back to the `#[not_found]` variant.
    assert_eq!(AppRoute::resolve_url("/nope"), Some(AppRoute::NotFound { raw_url: "/nope".to_owned() }));
}

#[test]
fn derive_query_round_trips() {
    let route = AppRoute::Search {
        q: "hello world".to_owned(),
        page: Some(2),
        tag: vec!["rust".to_owned(), "ui".to_owned()],
    };
    assert_eq!(route.to_url(), "/search?q=hello+world&page=2&tag=rust&tag=ui");
    assert_eq!(AppRoute::from_url("/search?q=hello+world&page=2&tag=rust&tag=ui"), Some(route));

    // Optional/repeated defaults when absent.
    assert_eq!(
        AppRoute::from_url("/search?q=rust"),
        Some(AppRoute::Search {
            q: "rust".to_owned(),
            page: None,
            tag: Vec::new(),
        })
    );
}

#[test]
fn redirect_target_route_matches_via_resolver() {
    // Build a router and use the derive `resolve_url` as a manual redirect
    // resolver to navigate the legacy URL to its typed target through the
    // public `MemoryAviator` surface.
    let router = Router::new().push(Router::with_path("users/<id>").outlet("page@user", || UserPage));
    let av = aviator(router);

    let target = AppRoute::resolve_url("/u/42").expect("legacy url should resolve");
    av.goto(&target.to_url()).unwrap();

    assert_eq!(av.current().as_deref(), Some("/users/42"));
    assert!(av.truck.contains_stuff_key("page", "user"));
    let locator = av.truck.borrow_mut().scrape::<Locator>().unwrap();
    assert_eq!(locator.params().get_untracked().get("id").map(String::as_str), Some("42"));
}

#[cfg(feature = "web-ssr")]
#[test]
fn ssr_aviator_with_redirects_rewrites_legacy_url() {
    use glory_routing::aviators::ServerAviator;

    let router = Router::new().push(Router::with_path("users/<id>").outlet("page@user", || UserPage));
    let catcher: Rc<dyn Handler> = Rc::new(|truck: Rc<RefCell<Truck>>| truck.insert_stuff("page@404", NotFoundPage));
    let av = ServerAviator::new(router, move |truck: Rc<RefCell<Truck>>| catcher.handle(truck)).with_redirects::<AppRoute>();
    av.truck.borrow_mut().inject(Locator::new());

    // The legacy `/u/42` URL must be rewritten to `/users/42` before matching.
    av.goto("/u/42").unwrap();

    assert!(av.truck.contains_stuff_key("page", "user"));
    let locator = av.truck.borrow_mut().scrape::<Locator>().unwrap();
    assert_eq!(&*locator.path().get_untracked(), "/users/42");
}
