use glory::{
    Widget,
    routing::{self, Location, Params, RouteDef, RouteKey, RouteViews, RouterHandle, Routes},
    ui::{input, text, view, widget},
};
pub const ROOT: RouteKey<()> = RouteKey::new("root");
pub const PAGE: RouteKey<Params> = RouteKey::new("page");
pub const HOME: RouteKey<()> = RouteKey::new("home");

fn navigation_result(result: routing::Result<routing::Navigation>) {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    browser::record_link_result(result);
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    result.unwrap();
}
pub fn routes() -> Routes {
    Routes::builder()
        .root(
            RouteDef::layout(ROOT, "")
                .child(RouteDef::new(HOME, ""))
                .child(RouteDef::new(PAGE, "page/:id")),
        )
        .finish()
        .unwrap()
}
pub fn page(routes: Routes, router: RouterHandle) -> impl Widget {
    RouteViews::new(routes)
        .route(ROOT, |ctx| {
            widget(move |scope| {
                let router = ctx.router();
                view()
                    .class("shell")
                    .fill((
                        text().content("Routing demo"),
                        input().class("shell-editor").value("initial"),
                        routing::link_with(
                            scope,
                            router.clone(),
                            Location::parse("/page/1").unwrap(),
                            navigation_result,
                        )
                        .unwrap()
                        .label("One"),
                        routing::link_with(
                            scope,
                            router,
                            Location::parse("/page/2").unwrap(),
                            navigation_result,
                        )
                        .unwrap()
                        .label("Two"),
                        ctx.outlet(),
                        view()
                            .style(
                                glory::style::Style::new().height(glory::style::Length::Px(1800.0)),
                            )
                            .fill(text().content("scroll space")),
                        text().content("anchor target").semantics(Some(Box::new(
                            glory::render::Semantics {
                                id: Some(glory::render::SemanticId::new("details").unwrap()),
                                ..Default::default()
                            },
                        ))),
                    ))
                    .show_in(scope)
            })
        })
        .unwrap()
        .route(HOME, |_| text().content("Home"))
        .unwrap()
        .route(PAGE, |ctx| {
            widget(move |scope| {
                let ctx = ctx.clone();
                text()
                    .class("route-value")
                    .content(scope.bond(move || {
                        let params = ctx.read_params().unwrap();
                        let snapshot = ctx.router().snapshot().unwrap();
                        format!(
                            "id={};tab={};hash={}",
                            params.required::<String>("id").unwrap(),
                            snapshot
                                .location()
                                .query()
                                .optional::<String>("tab")
                                .unwrap()
                                .unwrap_or_default(),
                            snapshot.location().fragment().unwrap_or_default()
                        )
                    }))
                    .show_in(scope)
            })
        })
        .unwrap()
        .fallback(|router| {
            widget(move |scope| {
                text()
                    .content(scope.bond({
                        let router = router.clone();
                        move || format!("Not found: {}", router.snapshot().unwrap().location())
                    }))
                    .show_in(scope)
            })
        })
        .view(router)
        .unwrap()
}

#[cfg(all(feature = "web", target_arch = "wasm32"))]
mod browser {
    use super::*;
    use std::cell::{Cell, RefCell};
    use wasm_bindgen::prelude::*;
    thread_local! {
        static APP:RefCell<Option<glory::web::routing::RoutingApp>>=const{RefCell::new(None)};
        static ASYNC_GUARD:Cell<bool>=const{Cell::new(false)};
        static GUARD_MODE:Cell<u8>=const{Cell::new(0)};
        static GUARD_CALLS:Cell<u32>=const{Cell::new(0)};
        static GUARD_DROPS:Cell<u32>=const{Cell::new(0)};
        static LINK_RESULTS:RefCell<Vec<String>>=const{RefCell::new(Vec::new())};
    }
    pub(super) fn record_link_result(result: routing::Result<routing::Navigation>) {
        LINK_RESULTS.with_borrow_mut(|results| {
            assert!(results.len() < 64, "bounded acceptance result log");
            results.push(format!("{result:?}"));
        });
    }
    /// Explicit acceptance controls; ordinary demo startup installs no guards.
    #[wasm_bindgen]
    pub fn async_guard_mode(mode: u8) -> Result<(), JsValue> {
        if mode > 2 {
            return Err(JsValue::from_str("guard mode must be allow/block/pending"));
        }
        GUARD_MODE.set(mode);
        if ASYNC_GUARD.get() {
            return Ok(());
        }
        APP.with_borrow(|app| {
            app.as_ref().unwrap().router().async_guard(|_| {
                GUARD_CALLS.set(GUARD_CALLS.get() + 1);
                let mode = GUARD_MODE.get();
                Box::pin(async move {
                    struct Receipt;
                    impl Drop for Receipt {
                        fn drop(&mut self) {
                            GUARD_DROPS.set(GUARD_DROPS.get() + 1);
                        }
                    }
                    let _receipt = Receipt;
                    match mode {
                        0 => Ok(routing::Decision::Allow),
                        1 => Ok(routing::Decision::Block),
                        _ => std::future::pending().await,
                    }
                })
            })
        })
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
        ASYNC_GUARD.set(true);
        Ok(())
    }
    #[wasm_bindgen]
    pub fn guard_calls() -> u32 {
        GUARD_CALLS.get()
    }
    #[wasm_bindgen]
    pub fn guard_drops() -> u32 {
        GUARD_DROPS.get()
    }
    #[wasm_bindgen]
    pub fn link_results() -> String {
        LINK_RESULTS.with_borrow(|results| results.join("\n"))
    }
    #[wasm_bindgen(start)]
    pub fn start() -> Result<(), JsValue> {
        let document = web_sys::window().unwrap().document().unwrap();
        let root = document.get_element_by_id("app").unwrap();
        let routes = routes();
        let views = routes.clone();
        let on_error =
            |error: glory::routing::Error| web_sys::console::error_1(&error.to_string().into());
        let app = if let Some(seed) = document.get_element_by_id("route-seed") {
            let seed =
                glory::routing::Bootstrap::from_payload(&seed.text_content().unwrap()).unwrap();
            glory::web::routing::hydrate_with(
                root,
                routes,
                &seed,
                "demo-v1",
                move |_, router| page(views, router),
                on_error,
            )
        } else {
            glory::web::routing::launch_with(
                root,
                routes,
                move |_, router| page(views, router),
                on_error,
            )
        }
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
        APP.with(|slot| *slot.borrow_mut() = Some(app));
        Ok(())
    }
    #[wasm_bindgen]
    pub fn navigate(target: &str, replace: bool, force: bool) -> Result<(), JsValue> {
        APP.with(|slot| {
            let slot = slot.borrow();
            let router = slot.as_ref().unwrap().router();
            let location = Location::parse(target).unwrap();
            if replace {
                router.replace(location)
            } else if force {
                router.push_entry(location)
            } else {
                router.push(location)
            }
        })
        .map(|_| ())
        .map_err(|error| JsValue::from_str(&error.to_string()))
    }
    #[wasm_bindgen]
    pub fn stop() {
        APP.with(|slot| slot.borrow_mut().take());
    }
}
