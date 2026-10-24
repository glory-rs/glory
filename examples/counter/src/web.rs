//! Browser host. Keep the WebApp guard alive until the page is unloaded.
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

thread_local! {
    static APP: RefCell<Option<glory::web::WebApp>> = const { RefCell::new(None) };
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let root = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("app"))
        .ok_or_else(|| JsValue::from_str("missing #app mount element"))?;
    let app = glory::web::launch(root, crate::Counter)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    APP.with(|slot| *slot.borrow_mut() = Some(app));
    Ok(())
}
