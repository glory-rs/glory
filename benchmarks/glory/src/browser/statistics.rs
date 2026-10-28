//! Benchmark observations and optional acceptance exports.
use super::APP;
use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub fn stats() -> String {
    APP.with(|slot|{let slot=slot.borrow();let demo=slot.as_ref().unwrap();let d=demo.app.stats();let g=demo.app.runtime().stats();let rows=demo.model.rows.len_untracked();
        format!("{{\"profile\":\"html-display-keyed-v2\",\"rows\":{rows},\"labels\":{rows},\"instances\":{},\"targets\":{},\"records\":{},\"graph\":{},\"owners\":{}}}",d.instances,d.targets,d.records,g.nodes,g.owners)
    })
}
#[cfg(feature = "acceptance")]
#[wasm_bindgen]
pub fn flush_template() -> bool {
    APP.with(|s| s.borrow().as_ref().unwrap().app.flush().is_ok())
}
#[cfg(feature = "acceptance")]
#[wasm_bindgen]
pub fn close_template() -> String {
    APP.with(|slot| {
        let slot = slot.borrow();
        let app = &slot.as_ref().unwrap().app;
        app.close().unwrap();
        let d = app.stats();
        let g = app.runtime().stats();
        format!(
            "{{\"instances\":{},\"targets\":{},\"records\":{},\"graph\":{},\"owners\":{}}}",
            d.instances, d.targets, d.records, g.nodes, g.owners
        )
    })
}
