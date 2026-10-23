use crate::streaming::State;
use glory::prelude::*;
use glory::resources::Limits;
use glory::web::WebApp;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

thread_local! { static APP: RefCell<Option<(WebApp,State,Resources)>> = const { RefCell::new(None) }; }
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
pub fn stream_hydrate() -> std::result::Result<(), JsValue> {
    let doc = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| js("document"))?;
    let root = doc
        .get_element_by_id("stream-root")
        .ok_or_else(|| js("root"))?;
    let data = doc
        .get_element_by_id("stream-data")
        .ok_or_else(|| js("stream not complete"))?;
    let state = Rc::new(RefCell::new(None));
    let load: crate::streaming::Load =
        Rc::new(|id| Box::pin(async move { Ok(format!("client {id}")) }));
    let (web, resources) =
        glory::web::hydrate_resources_with(root, &data, Limits::default(), |_, resources| {
            crate::streaming::widget(resources.clone(), load, state.clone())
        })
        .map_err(js)?;
    resources.discard_unused();
    let state = state.borrow_mut().take().expect("state");
    APP.with(|app| *app.borrow_mut() = Some((web, state, resources)));
    Ok(())
}
#[wasm_bindgen]
pub fn stream_stats() -> String {
    APP.with(|app| {
        let app = app.borrow();
        let Some((web, state, resources)) = app.as_ref() else {
            return "null".into();
        };
        let draft = js_sys::JSON::stringify(&state.draft.get().unwrap_or_default().into())
            .expect("JSON string")
            .as_string()
            .expect("string");
        format!(
            "{{\"loads\":{},\"draft\":{},\"checked\":{},\"count\":{},\"registered\":{},\"nodes\":{}}}",
            state.loads.get(),
            draft,
            state.checked.get().unwrap_or(false),
            state.count.get(),
            resources.len(),
            web.app().stats().nodes
        )
    })
}
#[wasm_bindgen]
pub fn stream_program(value: &str) -> std::result::Result<(), JsValue> {
    APP.with(|app| {
        let app = app.borrow();
        let (web, state, _) = app.as_ref().ok_or_else(|| js("unmounted"))?;
        state.draft.revise(|draft| *draft = Some(value.into()));
        web.app().flush().map_err(js)
    })
}

#[wasm_bindgen]
pub fn stream_rich_text() -> String {
    APP.with(|app| {
        app.borrow()
            .as_ref()
            .and_then(|(_, state, _)| state.rich.get())
            .map(|doc| doc.plain_text())
            .unwrap_or_default()
    })
}
