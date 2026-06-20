//! RT7: routing integration tests driven entirely through the public API.
//!
//! These tests build a [`Router`] and navigate with [`MemoryAviator`]'s public
//! `goto`/`back`/`forward`, then assert on the injected `Truck` (outlet stuffs
//! and stuff keys) and the published [`Locator`] state. `run_route` is private,
//! so navigation is always driven through the public `Aviator` surface.

use std::cell::RefCell;
use std::rc::Rc;

use glory_core::{Scope, Truck, Widget};
use glory_routing::aviators::MemoryAviator;
use glory_routing::{Aviator, Handler, Locator, Router, TruckExt};

// --- Widgets used as route goals / layouts ---------------------------------

#[derive(Debug)]
struct Shell;
impl Widget for Shell {
    fn build(&mut self, _ctx: &mut Scope) {}
}

#[derive(Debug)]
struct Dashboard;
impl Widget for Dashboard {
    fn build(&mut self, _ctx: &mut Scope) {}
}

#[derive(Debug)]
struct Settings;
impl Widget for Settings {
    fn build(&mut self, _ctx: &mut Scope) {}
}

#[derive(Debug)]
struct UserPage;
impl Widget for UserPage {
    fn build(&mut self, _ctx: &mut Scope) {}
}

#[derive(Debug)]
struct CatchAllPage;
impl Widget for CatchAllPage {
    fn build(&mut self, _ctx: &mut Scope) {}
}

#[derive(Debug)]
struct NotFound;
impl Widget for NotFound {
    fn build(&mut self, _ctx: &mut Scope) {}
}

// --- Helpers ----------------------------------------------------------------

/// Build a `MemoryAviator` over `router` with a 404 catcher, and inject a fresh
/// `Locator` so route matching can publish navigation state. Mirrors what the
/// `Enabler::enable` lifecycle does for a real host.
fn aviator(router: Router) -> MemoryAviator {
    let catcher: Rc<dyn Handler> = Rc::new(|truck: Rc<RefCell<Truck>>| truck.insert_stuff("page@404", NotFound));
    let av = MemoryAviator::new(router, move |truck: Rc<RefCell<Truck>>| catcher.handle(truck));
    av.truck.borrow_mut().inject(Locator::new());
    av
}

/// Read the current `Locator` path from the aviator's truck.
fn current_path(av: &MemoryAviator) -> String {
    let locator = av.truck.borrow_mut().scrape::<Locator>().unwrap();
    let path = locator.path().get_untracked().to_owned();
    // `scrape` removed the locator; re-inject so subsequent navigation works.
    av.truck.borrow_mut().inject(locator);
    path
}

/// Read a named path param from the current `Locator`.
fn current_param(av: &MemoryAviator, name: &str) -> Option<String> {
    let locator = av.truck.borrow_mut().scrape::<Locator>().unwrap();
    let value = locator.params().get_untracked().get(name).cloned();
    av.truck.borrow_mut().inject(locator);
    value
}

// --- Tests ------------------------------------------------------------------

#[test]
fn nested_layout_and_outlet_render_into_named_slots() {
    // `/app` mounts the Shell layout into the `shell` outlet; the matched
    // `/app/settings` child renders Settings into the `content` outlet.
    let router = Router::with_path("app")
        .layout("shell", || Shell)
        .push(Router::with_path("settings").outlet("content@settings", || Settings))
        .push(Router::with_path("dashboard").outlet("content@dashboard", || Dashboard));

    let av = aviator(router);
    av.goto("/app/settings").unwrap();

    let stuffs = av.truck.stuffs();
    let stuffs = stuffs.get();
    assert!(stuffs.contains_key("shell"), "parent layout outlet should be populated");
    assert!(stuffs.contains_key("content"), "child outlet should be populated");
    assert!(av.truck.contains_stuff_key("shell", std::any::type_name::<Shell>()));
    assert!(av.truck.contains_stuff_key("content", "settings"));
    assert_eq!(current_path(&av), "/app/settings");
}

#[test]
fn navigating_between_siblings_swaps_child_outlet_keeps_layout() {
    let router = Router::with_path("app")
        .layout("shell", || Shell)
        .push(Router::with_path("settings").outlet("content@settings", || Settings))
        .push(Router::with_path("dashboard").outlet("content@dashboard", || Dashboard));

    let av = aviator(router);

    av.goto("/app/settings").unwrap();
    assert!(av.truck.contains_stuff_key("content", "settings"));

    av.goto("/app/dashboard").unwrap();
    // The shared layout stays mounted; only the child outlet content swaps.
    assert!(av.truck.contains_stuff_key("shell", std::any::type_name::<Shell>()));
    assert!(av.truck.contains_stuff_key("content", "dashboard"));
    assert_eq!(current_path(&av), "/app/dashboard");
}

#[test]
fn dynamic_segment_is_captured_in_locator_params() {
    let router = Router::new().push(Router::with_path("users/<id>").outlet("page@user", || UserPage));

    let av = aviator(router);
    av.goto("/users/42").unwrap();

    assert!(av.truck.contains_stuff_key("page", "user"));
    assert_eq!(current_param(&av, "id").as_deref(), Some("42"));
    assert_eq!(current_path(&av), "/users/42");
}

#[test]
fn catch_all_matches_remaining_segments() {
    let router = Router::new().push(Router::with_path("files/<**rest>").outlet("page@files", || CatchAllPage));

    let av = aviator(router);
    av.goto("/files/docs/readme.md").unwrap();

    assert!(av.truck.contains_stuff_key("page", "files"));
    // A `<**rest>` catch-all stores its captured tail under the `**rest` key.
    assert_eq!(current_param(&av, "**rest").as_deref(), Some("docs/readme.md"));
}

#[test]
fn static_route_wins_over_catch_all_regardless_of_registration_order() {
    // Register the catch-all FIRST. Specificity ranking must still prefer the
    // static `users` route.
    let router = Router::new()
        .push(Router::with_path("<**rest>").outlet("page@catch", || CatchAllPage))
        .push(Router::with_path("users").outlet("page@static", || Settings));

    let av = aviator(router);
    av.goto("/users").unwrap();

    assert!(
        av.truck.contains_stuff_key("page", "static"),
        "static route should win over earlier catch-all"
    );

    // A non-static path falls through to the catch-all.
    av.goto("/anything/else").unwrap();
    assert!(
        av.truck.contains_stuff_key("page", "catch"),
        "unmatched path should fall to the catch-all"
    );
}

#[test]
fn unmatched_route_falls_to_catcher() {
    let router = Router::new().push(Router::with_path("home").outlet("page@home", || Dashboard));

    let av = aviator(router);
    av.goto("/does-not-exist").unwrap();

    assert!(av.truck.contains_stuff_key("page", "404"), "no route should trigger the 404 catcher");
    assert_eq!(current_path(&av), "/does-not-exist");
}

#[test]
fn memory_aviator_back_and_forward_re_route() {
    let router = Router::new()
        .push(Router::with_path("a").outlet("page@a", || Settings))
        .push(Router::with_path("b").outlet("page@b", || Dashboard))
        .push(Router::with_path("c").outlet("page@c", || UserPage));

    let av = aviator(router);

    av.goto("/a").unwrap();
    av.goto("/b").unwrap();
    av.goto("/c").unwrap();
    assert_eq!(av.current().as_deref(), Some("/c"));
    assert!(av.truck.contains_stuff_key("page", "c"));

    // Back to /b -> re-route renders Dashboard.
    assert!(av.back().unwrap());
    assert_eq!(av.current().as_deref(), Some("/b"));
    assert!(av.truck.contains_stuff_key("page", "b"));
    assert_eq!(current_path(&av), "/b");

    // Back to /a.
    assert!(av.back().unwrap());
    assert_eq!(av.current().as_deref(), Some("/a"));
    assert!(av.truck.contains_stuff_key("page", "a"));

    // Already at the oldest entry.
    assert!(!av.back().unwrap());
    assert_eq!(av.current().as_deref(), Some("/a"));

    // Forward to /b again.
    assert!(av.forward().unwrap());
    assert_eq!(av.current().as_deref(), Some("/b"));
    assert!(av.truck.contains_stuff_key("page", "b"));
}

#[test]
fn memory_aviator_goto_truncates_forward_history() {
    let router = Router::new()
        .push(Router::with_path("a").outlet("page", || Settings))
        .push(Router::with_path("b").outlet("page", || Dashboard))
        .push(Router::with_path("c").outlet("page", || UserPage));

    let av = aviator(router);
    av.goto("/a").unwrap();
    av.goto("/b").unwrap();
    av.back().unwrap(); // cursor at /a, /b is a forward entry

    // Navigating elsewhere drops the forward entry.
    av.goto("/c").unwrap();
    assert_eq!(av.current().as_deref(), Some("/c"));
    assert!(!av.forward().unwrap(), "forward history should have been truncated by goto");

    // Back now returns to /a (not /b, which was discarded).
    assert!(av.back().unwrap());
    assert_eq!(av.current().as_deref(), Some("/a"));
}
