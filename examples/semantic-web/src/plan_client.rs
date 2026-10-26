use crate::plan_form::State;
use glory::web::WebApp;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
type Result<T = ()> = std::result::Result<T, JsValue>;
thread_local! { static APP: RefCell<Option<(WebApp, State)>> = const { RefCell::new(None) }; }
pub(crate) fn clear() {
    drop(APP.with(|app| app.borrow_mut().take()));
}
fn js(error: impl std::fmt::Display) -> JsValue {
    error.to_string().into()
}
#[wasm_bindgen]
pub fn plan_mount(hydrate: bool) -> Result {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(if hydrate { "plan-ssr" } else { "app" }))
        .ok_or_else(|| js("missing plan root"))?;
    let mut state = None;
    let build = |scope: &glory::ui::Scope| {
        let (widget, source) = crate::plan_form::ui(scope);
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
pub fn plan_edit(command: &str) -> Result {
    APP.with(|app| {
        let app = app.borrow();
        let (web, state) = app.as_ref().ok_or_else(|| js("unmounted"))?;
        match command {
            "add" => state
                .rows
                .revise(|rows| rows.push(rows.iter().max().copied().unwrap_or(0) + 1)),
            "swap" => state.rows.revise(|rows| rows.swap(0, 1)),
            "clear" => state.rows.revise(Vec::clear),
            "caption" => state.caption.revise(|v| v.push('!')),
            "value" => state.value.revise(|v| *v = "program🙂".into()),
            "check" => state.checked.revise(|v| *v = !*v),
            _ => return Err(js("unknown command")),
        }
        web.app().flush().map_err(js)
    })
}

/// Exercise partial retirement of a shared physical instance, then reuse the
/// retired semantic slots while the rest of that instance remains live.
#[wasm_bindgen]
pub fn plan_parts_lifecycle() -> Result {
    use glory::prelude::*;
    use glory::render::{Change, Descriptor, Patch, Props, Query, QueryValue};
    use std::{cell::Cell, rc::Rc};
    clear();
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.get_element_by_id("app").unwrap();
    let web = glory::web::launch(root.clone(), group()).map_err(js)?;
    let app = web.app();
    let owner = app.scope().child().map_err(js)?;
    let declaration = plan(
        view().class("split-plan").fill((
            text().class("split-first").content("first").mark("first"),
            button()
                .class("split-second")
                .label("second")
                .mark("second"),
        )),
    )
    .map_err(js)?;
    let first = declaration.part("first").map_err(js)?;
    let second = declaration.part("second").map_err(js)?;
    let instance = owner.instantiate(&declaration).map_err(js)?;
    let target_owner = app.scope().child().map_err(js)?;
    let target = target_owner
        .create(Descriptor::new(Props::View))
        .map_err(js)?;
    owner
        .place(instance.node(&first).map_err(js)?, Some(target), None)
        .map_err(js)?;
    app.flush().map_err(js)?;
    let removed = root.query_selector(".split-first")?.unwrap();
    let survivor = root.query_selector(".split-second")?.unwrap();
    target_owner.dispose();
    app.flush().map_err(js)?;
    assert!(!removed.is_connected());
    assert!(instance.text(&first, "retired".to_owned()).is_err());
    instance.label(&second, "survived".to_owned()).map_err(js)?;
    let replacement = owner.instantiate(&declaration).map_err(js)?;
    replacement.text(&first, "fresh".to_owned()).map_err(js)?;
    instance
        .class(&second, "split-second updated".to_owned())
        .map_err(js)?;
    app.flush().map_err(js)?;
    assert_eq!(survivor.text_content().as_deref(), Some("survived"));
    assert_eq!(removed.text_content().as_deref(), Some("first"));
    assert!(
        survivor.is_same_node(
            root.query_selector(".updated")?
                .as_ref()
                .map(|e| e.as_ref())
        )
    );
    instance
        .patch(&second, Patch::Classes(Change::Clear))
        .map_err(js)?;
    app.flush().map_err(js)?;
    assert!(!survivor.has_attribute("class"));
    let replied = Rc::new(Cell::new(false));
    let seen = replied.clone();
    app.request(
        Query::Bounds(instance.node(&second).map_err(js)?),
        move |reply| {
            assert!(matches!(reply, Ok(QueryValue::Bounds(rect)) if rect.width > 0.0));
            seen.set(true);
        },
    )
    .map_err(js)?;
    app.pump().map_err(js)?;
    assert!(replied.get());
    let parked_plan = plan(text().content("parked").class("parked-plan")).map_err(js)?;
    let parked = owner
        .mount_with(|| {
            widget(move |scope| {
                scope.instantiate(&parked_plan)?;
                Ok(())
            })
        })
        .map_err(js)?;
    app.flush().map_err(js)?;
    let parked_node = root.query_selector(".parked-plan")?.unwrap();
    parked.suspend().map_err(js)?;
    app.flush().map_err(js)?;
    assert!(!parked_node.is_connected());
    parked.place_before(None).map_err(js)?;
    app.flush().map_err(js)?;
    assert!(
        parked_node.is_same_node(
            root.query_selector(".parked-plan")?
                .as_ref()
                .map(|e| e.as_ref())
        )
    );
    owner.dispose();
    app.flush().map_err(js)?;
    assert!(root.query_selector(".split-plan")?.is_none());
    assert_eq!(app.stats().queries, 0);
    drop(web);
    assert!(!root.has_child_nodes());
    Ok(())
}
