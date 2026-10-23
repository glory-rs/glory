use crate::query_form::State;
use glory::web::WebApp;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

thread_local! {static APP:RefCell<Option<(WebApp,State)>>=const{RefCell::new(None)};}
pub(crate) fn clear() {
    drop(APP.with(|app| app.borrow_mut().take()));
}
#[wasm_bindgen]
pub fn query_mount(hydrate: bool) -> Result<(), JsValue> {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(if hydrate { "query-ssr" } else { "app" }))
        .ok_or_else(|| JsValue::from_str("query root missing"))?;
    let mut state = None;
    let build = |scope: &glory::ui::Scope| {
        let (widget, value) = crate::query_form::ui(scope);
        state = Some(value);
        widget
    };
    let web = if hydrate {
        glory::web::hydrate_with(root, build).map_err(|e| JsValue::from_str(&e.to_string()))?
    } else {
        glory::web::launch_with(root, build).map_err(|e| JsValue::from_str(&e.to_string()))?
    };
    APP.with(|app| *app.borrow_mut() = Some((web, state.unwrap())));
    Ok(())
}
#[wasm_bindgen]
pub fn query_stats() -> String {
    APP.with(|app| {
        let app = app.borrow();
        let Some((web, state)) = app.as_ref() else { return "null".into() };
        let stats = web.app().runtime_stats();
        let ui = web.app().stats();
        let handles = state.last.get();
        let value = handles.and_then(|(_,v)|v.try_get().ok()).map(|v|v.to_string()).unwrap_or_else(||"null".into());
        format!(
            "{{\"nodes\":{},\"slots\":{},\"owners\":{},\"fields\":{},\"queries\":{},\"queryOwners\":{},\"queryTargets\":{},\"fieldAlive\":{},\"derivedAlive\":{},\"derivedValue\":{},\"sent\":{},\"called\":{},\"dropped\":{},\"count\":{},\"payload\":{}}}",
            stats.nodes, stats.node_slots, stats.owner_slots, stats.fields, ui.queries, ui.query_owners, ui.query_targets,
            handles.is_some_and(|(v,_)|v.is_alive()), handles.is_some_and(|(_,v)|v.is_alive()), value,
            state.sent.get(), state.called.get(), state.dropped.get(), state.model.with(|v|v.0), state.model.with(|v|v.1.as_ptr() as usize)
        )
    })
}
