use glory::render::conformance::flex_cases;
use glory::render::{Change, Patch, Props};
use glory::web::WebApp;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

type Result<T = ()> = std::result::Result<T, JsValue>;
thread_local! { static APP: RefCell<Option<WebApp>> = const { RefCell::new(None) }; }
pub(crate) fn clear() {
    let old = APP.with(|app| app.borrow_mut().take());
    drop(old);
}
fn js(error: impl std::fmt::Display) -> JsValue {
    error.to_string().into()
}
#[wasm_bindgen]
pub fn flex_mount(index: usize, hydrate: bool) -> Result {
    crate::browser::unmount();
    if index >= flex_cases().len() || (hydrate && index != 0) {
        return Err(js("invalid flex fixture index"));
    }
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(if hydrate { "flex-ssr" } else { "app" }))
        .ok_or_else(|| js("flex root missing"))?;
    let web = if hydrate {
        glory::web::hydrate_with(root, move |_| crate::flex_form::ui(index)).map_err(js)?
    } else {
        glory::web::launch(root, crate::flex_form::ui(index)).map_err(js)?
    };
    APP.with(|app| *app.borrow_mut() = Some(web));
    Ok(())
}
#[wasm_bindgen]
pub fn flex_cases_json() -> String {
    let values: Vec<_> = flex_cases()
        .iter()
        .enumerate()
        .map(|(index, case)| {
            let rects: Vec<_> = case
                .expected
                .iter()
                .map(|r| format!("[{},{},{},{}]", r.x, r.y, r.width, r.height))
                .collect();
            format!(
                "{{\"index\":{index},\"name\":\"{}\",\"text\":{},\"expected\":[{}]}}",
                case.name,
                case.text,
                rects.join(",")
            )
        })
        .collect();
    format!("[{}]", values.join(","))
}
#[wasm_bindgen]
pub fn flex_case(index: usize) -> Result {
    let cases = flex_cases();
    let case = cases.get(index).ok_or_else(|| js("invalid flex index"))?;
    APP.with(|app| {
        let app = app.borrow();
        let app = app.as_ref().ok_or_else(|| js("unmounted"))?.app();
        let root = app.tree().roots()[0];
        let parent = app.tree().node(root).map_err(js)?.children[0];
        let children = app.tree().node(parent).map_err(js)?.children.clone();
        if case.text
            || children.iter().any(|id| {
                !matches!(
                    app.tree().node(*id).map(|n| n.descriptor.view().props),
                    Ok(Props::View)
                )
            })
        {
            return Err(js("text fixture cases require an explicit remount"));
        }
        app.scope()
            .patch(
                parent,
                Patch::Style(Change::Set(Box::new(case.parent.clone()))),
            )
            .map_err(js)?;
        for id in children {
            app.scope()
                .patch(id, Patch::Style(Change::Set(Box::new(case.child.clone()))))
                .map_err(js)?;
        }
        app.flush().map_err(js)
    })
}
