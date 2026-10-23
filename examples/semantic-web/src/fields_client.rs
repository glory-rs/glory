use crate::fields_form::State;
use glory::web::WebApp;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

type Result<T = ()> = std::result::Result<T, JsValue>;
thread_local! { static APP: RefCell<Option<(WebApp, State)>> = const { RefCell::new(None) }; }
pub(crate) fn clear() {
    let old = APP.with(|app| app.borrow_mut().take());
    drop(old);
}
fn js(error: impl std::fmt::Display) -> JsValue {
    error.to_string().into()
}
#[wasm_bindgen]
pub fn fields_mount(hydrate: bool) -> Result {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(if hydrate { "fields-ssr" } else { "app" }))
        .ok_or_else(|| js("fields root missing"))?;
    let mut state = None;
    let build = |scope: &glory::ui::Scope| {
        let (widget, data) = crate::fields_form::ui(scope);
        state = Some(data);
        widget
    };
    let web = if hydrate {
        glory::web::hydrate_with(root, build).map_err(js)?
    } else {
        glory::web::launch_with(root, build).map_err(js)?
    };
    APP.with(|app| *app.borrow_mut() = Some((web, state.unwrap())));
    Ok(())
}
#[wasm_bindgen]
pub fn fields_write(kind: &str, value: String, burst: usize) -> Result {
    APP.with(|app| {
        let app = app.borrow();
        let (web, state) = app.as_ref().ok_or_else(|| js("unmounted"))?;
        for _ in 0..burst {
            match kind {
                "title" => state.title.revise(|v| *v = value.clone()),
                "draft" => state.draft.revise(|v| *v = value.clone()),
                "background" => state.background.revise(|v| *v += 1),
                "root" => state.model.revise(|m| m.title = value.clone()),
                _ => return Err(js("unknown field")),
            }
        }
        web.app().flush().map_err(js)
    })
}
#[wasm_bindgen]
pub fn fields_stats() -> String {
    APP.with(|app| {
        let app = app.borrow();
        let Some((web, state)) = app.as_ref() else { return "null".into() };
        let stats = web.app().runtime_stats();
        format!(
            "{{\"runs\":[{},{},{}],\"fields\":{},\"slots\":{},\"writes\":{},\"derived\":{},\"marked\":{},\"payload\":{},\"targetBuilds\":{},\"targetHits\":{},\"targetVisits\":{},\"targetCaches\":{},\"targetKeys\":{}}}",
            state.runs[0].get(),
            state.runs[1].get(),
            state.runs[2].get(),
            stats.fields,
            stats.field_slots,
            stats.source_writes,
            stats.derived_runs,
            stats.marked,
            state.model.with(|m| m.payload.as_ptr() as usize),
            stats.field_target_builds, stats.field_target_hits, stats.field_target_visits, stats.field_target_caches, stats.field_target_keys
        )
    })
}
