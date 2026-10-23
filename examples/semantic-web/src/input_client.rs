use crate::input_form::State;
use glory::render::{Change, Error, Event, Patch, Props};
use glory::web::WebApp;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

type Result<T = ()> = std::result::Result<T, JsValue>;
thread_local! { static APP: RefCell<Option<(WebApp, State)>> = const { RefCell::new(None) }; }
pub(crate) fn clear() {
    let old = APP.with(|app| app.borrow_mut().take());
    drop(old);
}
fn js(error: impl std::fmt::Display) -> JsValue {
    error.to_string().into()
}
#[wasm_bindgen]
pub fn input_mount(hydrate: bool) -> Result {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(if hydrate { "input-ssr" } else { "app" }))
        .ok_or_else(|| js("missing input root"))?;
    let mut state = None;
    let build = |scope: &glory::ui::Scope| {
        let (widget, source) = crate::input_form::ui(scope);
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
pub fn input_write(index: usize, value: String, revision: &str) -> Result {
    APP.with(|app| {
        let app = app.borrow();
        let (web, state) = app.as_ref().ok_or_else(|| js("unmounted"))?;
        if revision == "auto" {
            state
                .values
                .get(index)
                .ok_or_else(|| js("index"))?
                .revise(|v| *v = value);
        } else {
            let node = web
                .app()
                .tree()
                .node(web.app().tree().roots()[0])
                .map_err(js)?
                .children[index];
            web.app()
                .scope()
                .patch(
                    node,
                    Patch::Value {
                        value: Change::Set(value),
                        revision: revision.parse().map_err(js)?,
                    },
                )
                .map_err(js)?;
        }
        web.app().flush().map_err(js)
    })
}
#[wasm_bindgen]
pub fn input_event(index: usize, revision: &str, is_commit: bool) -> Result<String> {
    APP.with(|app| {
        let app = app.borrow();
        let (web, _) = app.as_ref().ok_or_else(|| js("unmounted"))?;
        let tree = web.app().tree();
        let node = tree.node(tree.roots()[0]).map_err(js)?.children[index];
        drop(tree);
        let revision = revision.parse().map_err(js)?;
        let event = if is_commit {
            Event::Commit {
                value: "forged".into(),
                revision,
            }
        } else {
            Event::ValueChanged {
                value: "forged".into(),
                composing: false,
                revision,
            }
        };
        match web.app().dispatch(node, event) {
            Ok(true) => Ok("accepted".into()),
            Ok(false) => Ok("stale".into()),
            Err(glory::ui::Error::Render(Error::InputRevision { .. })) => {
                Ok("future-rejected".into())
            }
            Err(error) => Err(js(error)),
        }
    })
}
#[wasm_bindgen]
pub fn input_stats() -> String {
    APP.with(|app| {
        let app = app.borrow();
        let Some((web, state)) = app.as_ref() else {
            return "null".into();
        };
        let tree = web.app().tree();
        let rows: js_sys::Array = tree
            .node(tree.roots()[0])
            .unwrap()
            .children
            .iter()
            .map(|id| {
                let Props::Input(input) = &tree.node(*id).unwrap().descriptor.props else {
                    panic!("input")
                };
                js_sys::Array::of2(
                    &input.value.as_str().into(),
                    &input.revision.to_string().into(),
                )
            })
            .collect();
        let raw: js_sys::Array = state
            .values
            .iter()
            .map(|value| JsValue::from(value.get()))
            .collect();
        format!(
            "{{\"model\":{},\"raw\":{},\"commits\":{},\"nodes\":{}}}",
            js_sys::JSON::stringify(&rows).unwrap().as_string().unwrap(),
            js_sys::JSON::stringify(&raw).unwrap().as_string().unwrap(),
            state.commits.get(),
            web.app().stats().nodes
        )
    })
}
