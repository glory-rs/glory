use crate::hydration::State;
use glory::web::WebApp;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
thread_local! { static APP: RefCell<Option<(WebApp, State)>> = const { RefCell::new(None) }; }
pub(crate) fn clear() {
    APP.with(|app| {
        let previous = app.borrow_mut().take();
        drop(previous);
    });
}
fn js(error: impl std::fmt::Display) -> JsValue {
    error.to_string().into()
}
#[wasm_bindgen]
pub fn hydrate_fixture() -> Result<(), JsValue> {
    let root = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("ssr"))
        .ok_or_else(|| js("SSR root missing"))?;
    let mut state = None;
    let web = glory::web::hydrate_with(root, |scope| {
        let (widget, source) = crate::hydration::ui(scope);
        state = Some(source);
        widget
    })
    .map_err(js)?;
    APP.with(|app| *app.borrow_mut() = Some((web, state.expect("hydrated state"))));
    Ok(())
}
#[wasm_bindgen]
pub fn hydrate_edit() -> Result<(), JsValue> {
    APP.with(|app| {
        let app = app.borrow();
        let (web, state) = app.as_ref().ok_or_else(|| js("unmounted"))?;
        state.empty.revise(|value| *value = "now populated".into());
        state.order.revise(|order| order.reverse());
        state.show.revise(|show| *show = !*show);
        web.app().flush().map_err(js)
    })
}
#[wasm_bindgen]
pub fn hydrate_seek(offset: f64) -> Result<(), JsValue> {
    APP.with(|app| {
        let app = app.borrow();
        let (web, state) = app.as_ref().ok_or_else(|| js("unmounted"))?;
        state.position.revise(|value| *value = offset);
        web.app().flush().map_err(js)
    })
}
#[wasm_bindgen]
pub fn hydrate_stats() -> String {
    APP.with(|app| {
        let app = app.borrow();
        let Some((web, state)) = app.as_ref() else { return "null".into() };
        let string = |value: String| {
            js_sys::JSON::stringify(&value.into())
                .expect("string JSON")
                .as_string()
                .expect("JSON string")
        };
        let metrics = state.stats.borrow();
        let metrics = metrics.as_ref().expect("list metrics");
        format!(
            "{{\"value\":{},\"multiline\":{},\"checked\":{},\"count\":{},\"rows\":{},\"slots\":{},\"logical\":{}}}",
            string(state.value.get()),
            string(state.multiline.get()),
            state.checked.get(),
            state.count.get(),
            metrics.rows,
            web.app().stats().node_slots,
            metrics.logical
        )
    })
}
