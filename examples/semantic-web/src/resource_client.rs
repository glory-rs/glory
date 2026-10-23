use crate::resource_hydration::State;
use glory::resources::{Limits, Resources};
use glory::web::WebApp;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

thread_local! {
    static APP: RefCell<Option<(WebApp, State, Resources)>> = const { RefCell::new(None) };
}
fn js(error: impl std::fmt::Display) -> JsValue {
    error.to_string().into()
}
pub(crate) fn clear() {
    APP.with(|app| {
        let previous = app.borrow_mut().take();
        drop(previous);
    });
}
#[wasm_bindgen]
pub fn resource_hydrate() -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| js("document missing"))?;
    let root = document
        .get_element_by_id("resource-ssr")
        .ok_or_else(|| js("root missing"))?;
    let data = document
        .get_element_by_id("resource-data")
        .ok_or_else(|| js("data missing"))?;
    let state = Rc::new(RefCell::new(None));
    let (web, resources) =
        glory::web::hydrate_resources_with(root, &data, Limits::default(), |_, resources| {
            crate::resource_hydration::widget(resources.clone(), state.clone())
        })
        .map_err(js)?;
    resources.discard_unused();
    let state = state.borrow_mut().take().expect("state");
    APP.with(|app| *app.borrow_mut() = Some((web, state, resources)));
    Ok(())
}
#[wasm_bindgen]
pub fn resource_snapshot_key(key: u64) -> Result<(), JsValue> {
    APP.with(|app| {
        let app = app.borrow();
        let (web, state, _) = app.as_ref().ok_or_else(|| js("unmounted"))?;
        state.key.revise(|value| *value = key);
        web.app().flush().map_err(js)
    })
}
#[wasm_bindgen]
pub fn resource_snapshot_stats() -> String {
    APP.with(|app| {
        let app = app.borrow();
        let Some((web, state, resources)) = app.as_ref() else {
            return "null".into();
        };
        format!(
            "{{\"loads\":{},\"registered\":{},\"remaining\":{},\"tasks\":{}}}",
            state.loads.get(),
            resources.len(),
            resources.remaining(),
            web.app().runtime_stats().tasks
        )
    })
}
