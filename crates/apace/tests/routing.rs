#![cfg(all(feature = "routing", not(target_arch = "wasm32")))]
use glory_apace::routing::{NativeEvent, NativeNavigation};
use glory_routing::{
    Location, MemoryHistory, Params, RouteDef, RouteKey, RouteViews, Router, Routes,
};
use glory_ui::{App, Widget, text, view, widget};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[test]
fn shared_router_updates_a_real_native_document_and_preserves_layout_through_cancelled_back() {
    let renderer = Rc::new(RefCell::new(None));
    let capture = renderer.clone();
    let app = App::new(move |runtime| {
        let renderer = glory_apace::Renderer::new(runtime);
        *capture.borrow_mut() = Some(renderer.clone());
        renderer
    });
    let handle = Rc::new(RefCell::new(None));
    let capture = handle.clone();
    let builds = Rc::new(Cell::new(0));
    let calls = builds.clone();
    app.mount(widget(move |scope| {
        const ROOT: RouteKey<()> = RouteKey::new("root");
        const PAGE: RouteKey<Params> = RouteKey::new("page");
        let routes = Routes::builder()
            .root(RouteDef::layout(ROOT, "").child(RouteDef::new(PAGE, "page/:id")))
            .finish()
            .unwrap();
        let router = Router::new(
            scope,
            routes.clone(),
            Rc::new(MemoryHistory::new(Location::parse("/page/1").unwrap())),
        )
        .unwrap();
        *capture.borrow_mut() = Some(router.handle());
        let calls = calls.clone();
        RouteViews::new(routes)
            .route(ROOT, move |ctx| {
                calls.set(calls.get() + 1);
                view().fill((text().content("native-shell"), ctx.outlet()))
            })
            .unwrap()
            .route(PAGE, |ctx| {
                widget(move |scope| {
                    text()
                        .content(scope.bond({
                            let ctx = ctx.clone();
                            move || ctx.read_params().unwrap().required::<String>("id").unwrap()
                        }))
                        .show_in(scope)
                })
            })
            .unwrap()
            .view(router.handle())
            .unwrap()
            .show_in(scope)
    }))
    .unwrap();
    let handle = handle.borrow().as_ref().unwrap().clone();
    let mut navigation = NativeNavigation::new(handle.clone());
    navigation
        .dispatch(NativeEvent::Open(Location::parse("/page/2").unwrap()))
        .unwrap();
    app.flush().unwrap();
    assert!(navigation.begin_back().unwrap());
    navigation.cancel_back();
    app.flush().unwrap();
    assert_eq!(handle.current().unwrap().location().path(), "/page/2");
    navigation.begin_back().unwrap();
    navigation.finish_back().unwrap();
    app.flush().unwrap();
    assert_eq!(handle.current().unwrap().location().path(), "/page/1");
    assert_eq!(builds.get(), 1);
    let content = renderer.borrow().as_ref().unwrap().inspect(|doc| {
        doc.find_element_by_tag_name(&"body".into())
            .unwrap()
            .text_content()
    });
    assert!(content.contains("native-shell"));
    assert!(content.contains('1'));
    app.clear().unwrap();
    assert!(!handle.is_alive());
}

#[cfg(feature = "shell")]
#[test]
fn native_window_queues_back_input_until_observation() {
    use std::task::{Context, Waker};
    let window = glory_apace::shell::WindowApp::new();
    let handle = Rc::new(RefCell::new(None));
    let capture = handle.clone();
    window
        .mount(widget(move |scope| {
            let routes = Routes::builder()
                .root(RouteDef::new(RouteKey::<Params>::new("page"), "*path"))
                .finish()
                .unwrap();
            let router = Router::new(
                scope,
                routes,
                Rc::new(MemoryHistory::new(Location::parse("/").unwrap())),
            )
            .unwrap();
            router
                .handle()
                .push(Location::parse("/next").unwrap())
                .unwrap();
            *capture.borrow_mut() = Some(router.handle());
            text().content("window").show_in(scope)
        }))
        .unwrap();
    let router = handle.borrow().as_ref().unwrap().clone();
    window.attach_router(router.clone()).unwrap();
    window.queue_navigation(NativeEvent::Back).unwrap();
    assert_eq!(router.current().unwrap().location().path(), "/next");
    window.observe(&Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(router.current().unwrap().location().path(), "/");
    assert_eq!(
        window.take_navigation_result(),
        Some(Ok(glory_routing::Navigation::Committed))
    );
}
