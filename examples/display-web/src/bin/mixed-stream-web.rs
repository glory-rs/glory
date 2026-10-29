#[cfg(target_arch = "wasm32")]
#[path = "../stream_mixed.rs"]
mod fixture;
#[cfg(target_arch = "wasm32")]
mod browser {
    use super::fixture;
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;
    struct Demo {
        host: g::web::WebApp,
        model: fixture::Model,
    }
    thread_local! {static APP:RefCell<Option<Demo>>=const{RefCell::new(None)};}
    #[wasm_bindgen]
    pub fn mixed_stream_hydrate() -> Result<(), JsValue> {
        let doc = web_sys::window().unwrap().document().unwrap();
        let root = doc.get_element_by_id("stream-root").unwrap();
        let data = doc
            .get_element_by_id("stream-data")
            .ok_or_else(|| JsValue::from_str("stream is not ready"))?;
        let mut model = None;
        let (host, _) = g::web::hydrate_resources_with(
            root,
            &data,
            g::resources::Limits::default(),
            |scope, _| {
                let value = fixture::Model::new(scope, 3);
                let view = fixture::view(&value);
                model = Some(value);
                view
            },
        )
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
        APP.with(|app| {
            *app.borrow_mut() = Some(Demo {
                host,
                model: model.unwrap(),
            })
        });
        Ok(())
    }
    #[wasm_bindgen]
    pub fn mixed_stream_stats() -> String {
        APP.with(|app| {
            let app = app.borrow();
            let app = app.as_ref().unwrap();
            format!(
                "{{\"count\":{},\"drops\":{}}}",
                app.model.count.get(),
                app.model.drops.get()
            )
        })
    }
    #[wasm_bindgen]
    pub fn mixed_stream_toggle() {
        APP.with(|app| {
            let app = app.borrow();
            let app = app.as_ref().unwrap();
            app.model.visible.revise(|value| *value = !*value);
            app.host.app().settle(8).unwrap();
        });
    }
    #[wasm_bindgen]
    pub fn mixed_stream_close() {
        APP.with(|app| {
            let app = app.borrow_mut().take().unwrap();
            app.host.app().clear().unwrap();
        });
    }
}
fn main() {}
