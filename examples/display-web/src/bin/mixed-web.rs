#[cfg(target_arch = "wasm32")]
#[path = "../mixed.rs"]
mod mixed;
#[cfg(target_arch = "wasm32")]
mod browser {
    use super::mixed;
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;
    struct Demo {
        host: g::web::WebApp,
        model: mixed::Model,
    }
    thread_local! {static DEMO:RefCell<Option<Demo>>=const{RefCell::new(None)};}
    #[wasm_bindgen]
    pub fn mixed_mount(hydrate: bool) -> Result<(), JsValue> {
        let root = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id("mixed")
            .unwrap();
        let mut model = None;
        let factory = |scope: &g::ui::Scope| {
            let value = mixed::Model::new(scope);
            let view = mixed::view(&value);
            model = Some(value);
            view
        };
        let host = if hydrate {
            g::web::hydrate_with(root, factory).map_err(|e| JsValue::from_str(&format!("{e:?}")))
        } else {
            g::web::launch_with(root, factory).map_err(|e| JsValue::from_str(&format!("{e:?}")))
        }?;
        DEMO.with(|demo| {
            *demo.borrow_mut() = Some(Demo {
                host,
                model: model.unwrap(),
            })
        });
        Ok(())
    }
    #[wasm_bindgen]
    pub fn mixed_update() {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().unwrap();
            demo.model.visible.revise(|v| *v = !*v);
            demo.model.rows.swap(0, 1).unwrap();
            demo.model.vector.revise(|rows| {
                rows.swap(0, 1);
                rows[0].1.push('!');
            });
            demo.host.app().settle(8).unwrap();
        });
    }
    #[wasm_bindgen]
    pub fn mixed_stats() -> String {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().unwrap();
            let stats = demo.host.app().runtime().stats();
            format!(
                "{{\"count\":{},\"bubbled\":{},\"drops\":{},\"nodes\":{},\"owners\":{}}}",
                demo.model.count.get(),
                demo.model.bubbled.get(),
                demo.model.drops.get(),
                stats.nodes,
                stats.owners
            )
        })
    }
    #[wasm_bindgen]
    pub fn mixed_close() -> String {
        DEMO.with(|demo| {
            let demo = demo.borrow_mut().take().unwrap();
            demo.host.app().clear().unwrap();
            let runtime = demo.host.app().shared_runtime();
            let drops = demo.model.drops.clone();
            drop(demo);
            let stats = runtime.stats();
            format!(
                "{{\"nodes\":{},\"owners\":{},\"drops\":{}}}",
                stats.nodes,
                stats.owners,
                drops.get()
            )
        })
    }
}
fn main() {}
