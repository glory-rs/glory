//! Production browser driver acceptance for unrelated patches and motion state.
use glory::prelude::*;
use glory::render::{
    Change, NodeId, Patch, Semantics, TranslationKind, TranslationMotion, TranslationOffset,
};
use glory::web::WebApp;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

type Result<T = ()> = std::result::Result<T, JsValue>;
struct State {
    range: glory::ui::Mounted,
    web: WebApp,
    text: NodeId,
    input: NodeId,
    revision: u64,
}
thread_local! { static APP: RefCell<Option<State>> = const { RefCell::new(None) }; }
fn js(error: impl std::fmt::Display) -> JsValue {
    error.to_string().into()
}
pub(crate) fn clear() {
    drop(APP.with(|app| app.borrow_mut().take()));
}
#[wasm_bindgen]
pub fn translation_mount(count: usize) -> Result {
    if count > 1000 {
        return Err(js("translation fixture limit"));
    }
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("app"))
        .ok_or_else(|| js("translation root missing"))?;
    let range = Rc::new(RefCell::new(None));
    let mounted = range.clone();
    let web = glory::web::launch(
        root,
        view().fill((
            text().class("translation-text").content("initial"),
            input().class("translation-input").value("initial"),
            widget(move |scope| {
                *mounted.borrow_mut() = Some(
                    view()
                        .class("translation-parent")
                        .fill(
                            each((0..count).collect::<Vec<_>>())
                                .key(|key| *key)
                                .children(|_| {
                                    view().class("translation-node").semantics(Some(Box::new(
                                        Semantics {
                                            inert: true,
                                            translation: Some(TranslationMotion {
                                                kind: TranslationKind::Loop {
                                                    from: TranslationOffset::ZERO,
                                                    to: TranslationOffset::new(20., 0.),
                                                    period_millis: 1000,
                                                },
                                                paused: false,
                                            }),
                                            ..Default::default()
                                        },
                                    )))
                                }),
                        )
                        .mount(scope)?,
                );
                Ok(())
            }),
        )),
    )
    .map_err(js)?;
    let (text, input) = {
        let tree = web.app().tree();
        let children = &tree.node(tree.roots()[0]).map_err(js)?.children;
        (children[0], children[1])
    };
    let range = range
        .borrow_mut()
        .take()
        .ok_or_else(|| js("translation range missing"))?;
    APP.with(|app| {
        *app.borrow_mut() = Some(State {
            range,
            web,
            text,
            input,
            revision: 0,
        })
    });
    Ok(())
}
#[wasm_bindgen]
pub fn translation_update() -> Result {
    APP.with(|app| {
        let mut app = app.borrow_mut();
        let state = app.as_mut().ok_or_else(|| js("unmounted"))?;
        state.revision += 1;
        let value = format!("updated {}", state.revision);
        let app = state.web.app();
        app.scope()
            .patch(state.text, Patch::Text(Change::Set(value.clone())))
            .map_err(js)?;
        app.scope()
            .patch(
                state.input,
                Patch::Value {
                    value: Change::Set(value),
                    revision: state.revision + 1,
                },
            )
            .map_err(js)?;
        app.flush().map_err(js)
    })
}
#[wasm_bindgen]
pub fn translation_stage(stage: u8) -> Result {
    APP.with(|app| {
        let app = app.borrow();
        let state = app.as_ref().ok_or_else(|| js("unmounted"))?;
        let app = state.web.app();
        let parent = state.range.roots()[0];
        match stage {
            0 => app.scope().patch(
                parent,
                Patch::Style(Change::Set(Box::new(Style::new().display(Display::None)))),
            ),
            1 => app.scope().patch(parent, Patch::Style(Change::Clear)),
            2 => app.scope().patch(
                parent,
                Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                    hidden: true,
                    ..Default::default()
                })))),
            ),
            3 => app.scope().patch(parent, Patch::Semantics(Change::Clear)),
            4 => state.range.suspend(),
            5 => state.range.place_before(None),
            _ => return Err(js("translation stage")),
        }
        .map_err(js)?;
        app.flush().map_err(js)
    })
}
