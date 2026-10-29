use glory::{
    Widget,
    routing::{self, Location, Params, RouteDef, RouteKey, RouteViews, RouterHandle, Routes},
    ui::{input, text, view, widget},
};
pub const ROOT: RouteKey<()> = RouteKey::new("root");
pub const PAGE: RouteKey<Params> = RouteKey::new("page");
pub const HOME: RouteKey<()> = RouteKey::new("home");
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
            let router = ctx.router();
            view().class("shell").fill((
                text().content("Routing demo"),
                input().class("shell-editor").value("initial"),
                routing::link(
                    router.clone(),
                    Location::parse("/page/1").unwrap(),
                    |result| {
                        result.unwrap();
                    },
                )
                .label("One"),
                routing::link(router, Location::parse("/page/2").unwrap(), |result| {
                    result.unwrap();
                })
                .label("Two"),
                ctx.outlet(),
                view()
                    .style(glory::style::Style::new().height(glory::style::Length::Px(1800.0)))
                    .fill(text().content("scroll space")),
                text().content("anchor target").semantics(Some(Box::new(
                    glory::render::Semantics {
                        id: Some(glory::render::SemanticId::new("details").unwrap()),
                        ..Default::default()
                    },
                ))),
            ))
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
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;
    thread_local! {static APP:RefCell<Option<glory::web::routing::RoutingApp>>=const{RefCell::new(None)};}
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
