use glory::web::WebApp;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
thread_local! { static APP:RefCell<Option<(WebApp,crate::css_form::State)>>=const{RefCell::new(None)}; }
pub(crate) fn clear() {
    let old = APP.with(|app| app.borrow_mut().take());
    drop(old);
}
fn js(error: impl std::fmt::Display) -> JsValue {
    error.to_string().into()
}
#[wasm_bindgen]
pub fn css_mount(hydrate: bool) -> Result<(), JsValue> {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(if hydrate { "css-ssr" } else { "app" }))
        .ok_or_else(|| js("missing CSS root"))?;
    let mut state = None;
    let factory = |scope: &glory::ui::Scope| {
        let (widget, data) = crate::css_form::ui(scope);
        state = Some(data);
        widget
    };
    let app = if hydrate {
        glory::web::hydrate_with(root, factory).map_err(js)?
    } else {
        glory::web::launch_with(root, factory).map_err(js)?
    };
    APP.with(|slot| *slot.borrow_mut() = Some((app, state.expect("constructed"))));
    Ok(())
}
#[wasm_bindgen]
pub fn css_text() -> Result<String, JsValue> {
    glory::html::sheet(&crate::css_form::sheet(), glory::style::Profile::HTML).map_err(js)
}
#[wasm_bindgen]
pub fn css_update(enabled: bool, checked: bool, hot: bool) -> Result<(), JsValue> {
    APP.with(|slot| {
        let slot = slot.borrow();
        let (app, state) = slot.as_ref().ok_or_else(|| js("unmounted"))?;
        app.app()
            .runtime()
            .batch(|| {
                state.enabled.revise(|v| *v = enabled);
                state.checked.revise(|v| *v = checked);
                state.hot.revise(|v| *v = hot);
            })
            .map_err(js)
    })
}

#[wasm_bindgen]
pub fn css_type_wrap(wrap: bool) -> Result<(), JsValue> {
    APP.with(|slot| {
        let slot = slot.borrow();
        let (_, state) = slot.as_ref().ok_or_else(|| js("unmounted"))?;
        state.typography.revise(|value| {
            *value = if wrap {
                glory::style::Style::new()
                    .white_space(glory::style::WhiteSpace::PreWrap)
                    .font_weight(700.0)
            } else {
                glory::style::Style::new()
            }
        });
        Ok(())
    })
}
