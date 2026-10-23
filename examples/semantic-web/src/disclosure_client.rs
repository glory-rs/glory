use glory::web::WebApp;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
thread_local! {static APPS:RefCell<Vec<WebApp>>=const{RefCell::new(Vec::new())};}
pub(crate) fn clear() {
    drop(APPS.with(|apps| std::mem::take(&mut *apps.borrow_mut())));
}
#[wasm_bindgen]
pub fn disclosure_mount(root_id: &str, hydrate: bool, initial: bool) -> Result<(), JsValue> {
    let root = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .get_element_by_id(root_id)
        .ok_or_else(|| JsValue::from_str("missing root"))?;
    let app = if hydrate {
        glory::web::hydrate_with(root, move |scope| {
            crate::disclosure_form::ui(scope, initial).0
        })
        .map_err(|error| JsValue::from_str(&error.to_string()))?
    } else {
        glory::web::launch_with(root, move |scope| {
            crate::disclosure_form::ui(scope, initial).0
        })
        .map_err(|error| JsValue::from_str(&error.to_string()))?
    };
    APPS.with(|apps| apps.borrow_mut().push(app));
    Ok(())
}
