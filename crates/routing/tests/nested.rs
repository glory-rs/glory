#![cfg(feature = "widgets")]
use glory_routing::*;
use glory_ui::{App, Widget, text, view, widget};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const ROOT: RouteKey<()> = RouteKey::new("root");
const USER: RouteKey<Params> = RouteKey::new("user");
const SETTINGS: RouteKey<()> = RouteKey::new("settings");
fn location(value: &str) -> Location {
    Location::parse(value).unwrap()
}
fn routes() -> Routes {
    Routes::builder()
        .root(
            RouteDef::layout(ROOT, "")
                .child(RouteDef::new(USER, "users/:id"))
                .child(RouteDef::new(SETTINGS, "settings")),
        )
        .finish()
        .unwrap()
}
#[test]
fn siblings_and_parameters_retain_layout_and_page_identity_then_release_every_owner() {
    let app = App::memory();
    let baseline = app.runtime().stats();
    let layout_builds = Rc::new(Cell::new(0));
    let user_builds = Rc::new(Cell::new(0));
    let released = Rc::new(Cell::new(0));
    let handles = Rc::new(RefCell::new(None));
    let layout_calls = layout_builds.clone();
    let user_calls = user_builds.clone();
    let retired = released.clone();
    let handle_slot = handles.clone();
    app.mount(widget(move |scope| {
        let definitions = routes();
        let router = Router::new(
            scope,
            definitions.clone(),
            Rc::new(MemoryHistory::new(location("/users/1"))),
        )
        .unwrap();
        *handle_slot.borrow_mut() = Some(router.handle());
        let layout_calls = layout_calls.clone();
        let user_calls = user_calls.clone();
        let retired = retired.clone();
        let views =
            RouteViews::new(definitions)
                .route(ROOT, move |ctx| {
                    layout_calls.set(layout_calls.get() + 1);
                    view()
                        .class("shell")
                        .fill((text().content("header"), ctx.outlet()))
                })
                .unwrap()
                .route(USER, move |ctx| {
                    user_calls.set(user_calls.get() + 1);
                    let retired = retired.clone();
                    widget(move |scope| {
                        let retired = retired.clone();
                        scope.on_cleanup(move || retired.set(retired.get() + 1))?;
                        let params = ctx.params(scope.owner()).unwrap();
                        text()
                            .content(scope.bond(move || {
                                params.get().unwrap().required::<String>("id").unwrap()
                            }))
                            .show_in(scope)
                    })
                })
                .unwrap()
                .route(SETTINGS, |_| text().content("settings"))
                .unwrap();
        views.view(router.handle()).unwrap().show_in(scope)
    }))
    .unwrap();
    let handle = handles.borrow().as_ref().unwrap().clone();
    assert_eq!(layout_builds.get(), 1);
    assert_eq!(user_builds.get(), 1);
    let roots = app.tree().roots().to_vec();
    handle.push(location("/users/2?tab=b#detail")).unwrap();
    app.flush().unwrap();
    assert_eq!(layout_builds.get(), 1);
    assert_eq!(user_builds.get(), 1);
    assert_eq!(app.tree().roots(), roots);
    handle.push(location("/settings")).unwrap();
    app.flush().unwrap();
    assert_eq!(layout_builds.get(), 1);
    assert_eq!(released.get(), 1);
    handle.back().unwrap();
    app.flush().unwrap();
    assert_eq!(layout_builds.get(), 1);
    assert_eq!(user_builds.get(), 2);
    app.clear().unwrap();
    assert_eq!(released.get(), 2);
    assert!(!handle.is_alive());
    assert_eq!(app.runtime().stats().owners, baseline.owners);
}
#[test]
fn request_scoped_views_render_with_the_existing_ssr_renderer() {
    fn page(initial: &'static str) -> impl Widget {
        widget(move |scope| {
            let routes = routes();
            let router = Router::new(
                scope,
                routes.clone(),
                Rc::new(MemoryHistory::new(location(initial))),
            )
            .unwrap();
            RouteViews::new(routes)
                .route(ROOT, |ctx| {
                    view().fill((text().content("shell"), ctx.outlet()))
                })
                .unwrap()
                .route(USER, |ctx| {
                    text().content(ctx.read_params().unwrap().required::<String>("id").unwrap())
                })
                .unwrap()
                .route(SETTINGS, |_| text().content("settings"))
                .unwrap()
                .view(router.handle())
                .unwrap()
                .show_in(scope)
        })
    }
    let a = glory_ssr::render(page("/users/1")).unwrap();
    let b = glory_ssr::render(page("/users/2")).unwrap();
    assert!(a.contains("shell"));
    assert!(a.contains(">1<"));
    assert!(b.contains(">2<"));
}

#[test]
fn missing_or_duplicate_outlets_fail_and_explicit_parameter_keys_retire_old_contexts() {
    let bad = glory_ssr::render(widget(|scope| {
        let definitions = routes();
        let router = Router::new(
            scope,
            definitions.clone(),
            Rc::new(MemoryHistory::new(location("/users/1"))),
        )
        .unwrap();
        RouteViews::new(definitions)
            .route(ROOT, |_| text().content("missing outlet"))
            .unwrap()
            .route(USER, |_| text())
            .unwrap()
            .view(router.handle())
            .unwrap()
            .show_in(scope)
    }));
    assert!(bad.is_err());
    let duplicate = glory_ssr::render(widget(|scope| {
        let definitions = routes();
        let router = Router::new(
            scope,
            definitions.clone(),
            Rc::new(MemoryHistory::new(location("/users/1"))),
        )
        .unwrap();
        RouteViews::new(definitions)
            .route(ROOT, |ctx| view().fill((ctx.outlet(), ctx.outlet())))
            .unwrap()
            .route(USER, |_| text())
            .unwrap()
            .view(router.handle())
            .unwrap()
            .show_in(scope)
    }));
    assert!(duplicate.is_err());
    let app = App::memory();
    let handle = Rc::new(RefCell::new(None));
    let capture = handle.clone();
    let contexts = Rc::new(RefCell::new(Vec::new()));
    let stored = contexts.clone();
    app.mount(widget(move |scope| {
        let definitions = routes();
        let router = Router::new(
            scope,
            definitions.clone(),
            Rc::new(MemoryHistory::new(location("/users/1"))),
        )
        .unwrap();
        *capture.borrow_mut() = Some(router.handle());
        let stored = stored.clone();
        RouteViews::new(definitions)
            .route(ROOT, |ctx| view().fill(ctx.outlet()))
            .unwrap()
            .route(USER, move |ctx| {
                stored.borrow_mut().push(ctx);
                text()
            })
            .unwrap()
            .key_by(USER, |params| params.required::<String>("id").unwrap())
            .unwrap()
            .view(router.handle())
            .unwrap()
            .show_in(scope)
    }))
    .unwrap();
    let router = handle.borrow().as_ref().unwrap().clone();
    router.push(location("/users/2")).unwrap();
    app.flush().unwrap();
    assert_eq!(contexts.borrow().len(), 2);
    assert!(matches!(
        contexts.borrow()[0].read_params(),
        Err(Error::Gone)
    ));
    assert_eq!(
        contexts.borrow()[1]
            .read_params()
            .unwrap()
            .required::<String>("id")
            .unwrap(),
        "2"
    );
}
