use crate::owner_form::State;
use glory::web::WebApp;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

thread_local! {static APP:RefCell<Option<(WebApp,State)>>=const{RefCell::new(None)};}
pub(crate) fn clear() {
    drop(APP.with(|app| app.borrow_mut().take()));
}
#[wasm_bindgen]
pub fn owner_mount(hydrate: bool) -> Result<(), JsValue> {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(if hydrate { "owner-ssr" } else { "app" }))
        .ok_or_else(|| JsValue::from_str("owner root missing"))?;
    let mut state = None;
    let build = |scope: &glory::ui::Scope| {
        let (widget, data) = crate::owner_form::ui(scope);
        state = Some(data);
        widget
    };
    let web = if hydrate {
        glory::web::hydrate_with(root, build)
            .map_err(|error| JsValue::from_str(&error.to_string()))?
    } else {
        glory::web::launch_with(root, build)
            .map_err(|error| JsValue::from_str(&error.to_string()))?
    };
    APP.with(|app| *app.borrow_mut() = Some((web, state.unwrap())));
    Ok(())
}
#[wasm_bindgen]
pub fn owner_stats() -> String {
    APP.with(|app| {
        let app = app.borrow();
        let Some((web, state)) = app.as_ref() else { return "null".into() };
        let stats = web.app().runtime_stats();
        let handles = state.last.get();
        let value = handles.and_then(|(_,derived)|derived.try_get().ok()).map(|value|value.to_string()).unwrap_or_else(||"null".into());
        format!(
            "{{\"nodes\":{},\"slots\":{},\"owners\":{},\"fields\":{},\"targets\":{},\"fieldAlive\":{},\"derivedAlive\":{},\"derivedValue\":{},\"count\":{},\"payload\":{}}}",
            stats.nodes, stats.node_slots, stats.owner_slots, stats.fields, stats.field_target_caches,
            handles.is_some_and(|(field,_)|field.is_alive()), handles.is_some_and(|(_,derived)|derived.is_alive()), value,
            state.model.with(|value|value.0), state.model.with(|value|value.1.as_ptr() as usize)
        )
    })
}
