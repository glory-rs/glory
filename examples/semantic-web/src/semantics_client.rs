use glory::prelude::*;
use glory::render::{Action, Change, Descriptor, Error, NodeId, Patch, Props};
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
fn find(app: &App, class: &str) -> Result<NodeId> {
    let tree = app.tree();
    let mut pending = tree.roots().to_vec();
    while let Some(id) = pending.pop() {
        let node = tree.node(id).map_err(js)?;
        if node.descriptor.classes.iter().any(|value| value == class) {
            return Ok(id);
        }
        pending.extend(node.children.iter().copied());
    }
    Err(js("missing fixture node"))
}
#[wasm_bindgen]
pub fn semantics_mount(hydrate: bool) -> Result {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(if hydrate { "semantics-ssr" } else { "app" }))
        .ok_or_else(|| js("missing semantics root"))?;
    let web = if hydrate {
        glory::web::hydrate_with(root, |_| crate::semantics_form::ui()).map_err(js)?
    } else {
        glory::web::launch(root, crate::semantics_form::ui()).map_err(js)?
    };
    APP.with(|app| *app.borrow_mut() = Some(web));
    Ok(())
}
#[wasm_bindgen]
pub fn semantics_stage(stage: u8) -> Result<usize> {
    APP.with(|app| {
        let app = app.borrow();
        let web = app.as_ref().ok_or_else(|| js("unmounted"))?;
        let app = web.app();
        let classes = [
            "sem-panel",
            "sem-group",
            "sem-heading",
            "sem-button",
            "sem-input",
            "sem-check",
            "sem-image",
            "sem-link",
        ];
        let panel = find(app, "sem-panel")?;
        let editor = find(app, "sem-input")?;
        if stage == 4 {
            let mut rejected = 0;
            for class in [
                "sem-heading",
                "sem-inline",
                "sem-paragraph",
                "sem-button",
                "sem-input",
                "sem-check",
                "sem-image",
                "sem-decorative",
                "sem-link",
            ] {
                let id = find(app, class)?;
                let before = app.stats();
                if !matches!(
                    app.scope()
                        .patch(id, Patch::Role(Change::Set(Some(Role::List)))),
                    Err(glory::ui::Error::Render(Error::InvalidProps(_)))
                ) || app.stats() != before
                {
                    return Err(js("invalid role changed model/queue"));
                }
                rejected += 1;
            }
            for class in ["sem-inline", "sem-paragraph", "sem-decorative"] {
                let id = find(app, class)?;
                let before = app.stats();
                if !matches!(
                    app.scope()
                        .patch(id, Patch::Name(Change::Set(Some("ignored".into())))),
                    Err(glory::ui::Error::Render(Error::InvalidProps(_)))
                ) || app.stats() != before
                {
                    return Err(js("invalid name changed model/queue"));
                }
                rejected += 1;
            }
            let decorative = find(app, "sem-decorative")?;
            let before = app.stats();
            if app
                .scope()
                .patch(decorative, Patch::Focus(Change::Set(FocusMode::TabStop)))
                .is_ok()
                || app.scope().action(Action::Focus(decorative)).is_ok()
                || app.stats() != before
            {
                return Err(js("decorative focus accepted"));
            }
            rejected += 2;
            let mut invalid = Descriptor::new(Props::Input(Default::default()));
            invalid.role = Some(Role::List);
            if app.scope().create(invalid).is_ok() || app.stats() != before {
                return Err(js("invalid create consumed identity"));
            }
            rejected += 1;
            app.scope()
                .patch(
                    editor,
                    Patch::Name(Change::Set(Some("Recovered editor".into()))),
                )
                .map_err(js)?;
            app.flush().map_err(js)?;
            return Ok(rejected);
        }
        for class in classes {
            let id = find(app, class)?;
            let change = match stage {
                1 => Change::Set(Some(format!("Updated {class}"))),
                2 => Change::Keep,
                3 => Change::Clear,
                _ => return Err(js("stage")),
            };
            app.scope().patch(id, Patch::Name(change)).map_err(js)?;
        }
        match stage {
            1 => {
                app.scope()
                    .patch(panel, Patch::Role(Change::Set(Some(Role::Status))))
                    .map_err(js)?;
                app.scope()
                    .patch(editor, Patch::Focus(Change::Set(FocusMode::Programmatic)))
                    .map_err(js)?;
            }
            2 => {
                app.scope()
                    .patch(panel, Patch::Role(Change::Keep))
                    .map_err(js)?;
                app.scope()
                    .patch(editor, Patch::Focus(Change::Keep))
                    .map_err(js)?;
            }
            3 => {
                app.scope()
                    .patch(panel, Patch::Role(Change::Clear))
                    .map_err(js)?;
                app.scope()
                    .patch(editor, Patch::Focus(Change::Clear))
                    .map_err(js)?;
            }
            _ => unreachable!(),
        }
        let item = find(app, "sem-item")?;
        app.scope()
            .patch(
                item,
                Patch::Item(match stage {
                    1 => Change::Set(Some(ItemPosition {
                        index: 99_999_999,
                        count: None,
                    })),
                    2 => Change::Keep,
                    _ => Change::Clear,
                }),
            )
            .map_err(js)?;
        app.flush().map_err(js)?;
        Ok(0)
    })
}
