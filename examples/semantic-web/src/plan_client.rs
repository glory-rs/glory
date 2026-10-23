use crate::plan_form::State;
use glory::web::WebApp;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
type Result<T = ()> = std::result::Result<T, JsValue>;
thread_local! { static APP: RefCell<Option<(WebApp, State)>> = const { RefCell::new(None) }; }
pub(crate) fn clear() {
    drop(APP.with(|app| app.borrow_mut().take()));
}
fn js(error: impl std::fmt::Display) -> JsValue {
    error.to_string().into()
}
#[wasm_bindgen]
pub fn plan_mount(hydrate: bool) -> Result {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(if hydrate { "plan-ssr" } else { "app" }))
        .ok_or_else(|| js("missing plan root"))?;
    let mut state = None;
    let build = |scope: &glory::ui::Scope| {
        let (widget, source) = crate::plan_form::ui(scope);
        state = Some(source);
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
pub fn plan_edit(command: &str) -> Result {
    APP.with(|app| {
        let app = app.borrow();
        let (web, state) = app.as_ref().ok_or_else(|| js("unmounted"))?;
        match command {
            "add" => state
                .rows
                .revise(|rows| rows.push(rows.iter().max().copied().unwrap_or(0) + 1)),
            "swap" => state.rows.revise(|rows| rows.swap(0, 1)),
            "clear" => state.rows.revise(Vec::clear),
            "caption" => state.caption.revise(|v| v.push('!')),
            "value" => state.value.revise(|v| *v = "program🙂".into()),
            "check" => state.checked.revise(|v| *v = !*v),
            _ => return Err(js("unknown command")),
        }
        web.app().flush().map_err(js)
    })
}
