use glory::prelude::*;
use glory::render::{Change, Error, Patch};
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
pub fn size_css() -> String {
    crate::size_form::CSS.into()
}
#[wasm_bindgen]
pub fn size_mount(hydrate: bool) -> Result {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(if hydrate { "size-ssr" } else { "app" }))
        .ok_or_else(|| js("size root missing"))?;
    let web = if hydrate {
        glory::web::hydrate_with(root, |_| crate::size_form::ui()).map_err(js)?
    } else {
        glory::web::launch(root, crate::size_form::ui()).map_err(js)?
    };
    APP.with(|app| *app.borrow_mut() = Some(web));
    Ok(())
}
#[wasm_bindgen]
pub fn size_stage(stage: u8) -> Result<usize> {
    APP.with(|app| {
        let app = app.borrow();
        let app = app.as_ref().ok_or_else(|| js("unmounted"))?.app();
        let root = app.tree().roots()[0];
        let node = app.tree().node(root).map_err(js)?.children[0];
        if stage == 7 {
            let mut rejected = 0;
            for limit in [
                Limit::Px(-1.0),
                Limit::Px(f64::NAN),
                Limit::Percent(f64::INFINITY),
            ] {
                for style in [
                    Style::new().max_width(limit),
                    Style::new().max_height(limit),
                ] {
                    let before = (
                        app.stats(),
                        app.tree().node(node).map_err(js)?.descriptor.clone(),
                    );
                    if !matches!(
                        app.scope()
                            .patch(node, Patch::Style(Change::Set(Box::new(style)))),
                        Err(glory::ui::Error::Render(Error::InvalidProps(_)))
                    ) || app.stats() != before.0
                        || app.tree().node(node).map_err(js)?.descriptor != before.1
                    {
                        return Err(js("invalid limit changed model or queue"));
                    }
                    rejected += 1;
                }
            }
            return Ok(rejected);
        }
        let patch = match stage {
            5 => Change::Clear,
            6 => Change::Keep,
            stage => Change::Set(Box::new(
                crate::size_form::stage(stage).ok_or_else(|| js("invalid stage"))?,
            )),
        };
        app.scope().patch(node, Patch::Style(patch)).map_err(js)?;
        app.flush().map_err(js)?;
        Ok(0)
    })
}
