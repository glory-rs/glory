use crate::entries_form::State;
use glory::web::WebApp;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
type Result<T = ()> = std::result::Result<T, JsValue>;
thread_local! {static APP:RefCell<Option<(WebApp,State)>>=const{RefCell::new(None)};}
pub(crate) fn clear() {
    let old = APP.with(|app| app.borrow_mut().take());
    drop(old);
}
fn js(e: impl std::fmt::Display) -> JsValue {
    e.to_string().into()
}
#[wasm_bindgen]
pub fn entries_mount(hydrate: bool) -> Result {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(if hydrate { "entries-ssr" } else { "app" }))
        .ok_or_else(|| js("entries root missing"))?;
    let mut state = None;
    let build = |scope: &glory::ui::Scope| {
        let (widget, data) = crate::entries_form::ui(scope);
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
pub fn entries_write(kind: &str, value: String) -> Result {
    APP.with(|app| {
        let app = app.borrow();
        let (web, state) = app.as_ref().ok_or_else(|| js("unmounted"))?;
        match kind {
            "title" => state.title.try_revise(|v| *v = value).map_err(js)?,
            "reserve" => state.rows.revise(|m| m.reserve(10000)),
            "remove" => {
                state.rows.try_remove(&7).map_err(js)?;
            }
            "insert" => {
                state
                    .rows
                    .try_insert(7, crate::entries_form::row(7))
                    .map_err(js)?;
            }
            "append" => state.order.try_push(11).map_err(js)?,
            "pop" => {
                state.order.try_pop().map_err(js)?;
            }
            "prepend" => state.order.try_insert(0, 13).map_err(js)?,
            "remove_first" => {
                state.order.try_remove(0).map_err(js)?;
            }
            "swap" => state.order.revise(|order| order.swap(0, 1)),
            _ => return Err(js("unknown entry action")),
        }
        web.app().flush().map_err(js)
    })
}
#[wasm_bindgen]
pub fn entries_stats() -> String {
    APP.with(|app| {
        let app = app.borrow();
        let Some((web, state)) = app.as_ref() else { return "null".into() };
        let stats = web.app().runtime_stats();
        format!(
            "{{\"runs\":[{},{},{},{}],\"fields\":{},\"keys\":{},\"buckets\":{},\"writes\":{},\"payload9\":{},\"targetBuilds\":{},\"targetHits\":{},\"targetCaches\":{},\"targetKeys\":{},\"positions\":{}}}",
            state.runs[0].get(),
            state.runs[1].get(),
            state.runs[2].get(),
            state.runs[3].get(),
            stats.fields,
            stats.entry_keys,
            stats.entry_buckets,
            stats.source_writes,
            state.rows.with(|m| m[&9].payload.as_ptr() as usize),
            stats.field_target_builds, stats.field_target_hits, stats.field_target_caches, stats.field_target_keys, state.positions.get()
        )
    })
}
