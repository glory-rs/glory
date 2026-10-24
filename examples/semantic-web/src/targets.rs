//! The semantic target matrix exercised through a real browser document.
use glory::prelude::*;
use glory::render::{Error, EventKind, NodeId, Query};
use glory::style::{Display, Length, Style};
use glory::web::WebApp;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::prelude::*;

type Result<T = ()> = std::result::Result<T, JsValue>;
struct Demo {
    web: WebApp,
    nodes: Vec<NodeId>,
    responses: Rc<RefCell<Vec<String>>>,
    activations: Rc<Cell<usize>>,
}
thread_local! {
    static DEMO: RefCell<Option<Demo>> = const { RefCell::new(None) };
}
fn js(error: impl std::fmt::Display) -> JsValue {
    error.to_string().into()
}
pub(crate) fn clear() {
    let old = DEMO.with(|demo| demo.borrow_mut().take());
    drop(old);
}
#[wasm_bindgen]
pub fn target_mount() -> Result {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("app"))
        .ok_or_else(|| js("missing root"))?;
    let boxed = Style::new()
        .display(Display::Block)
        .width(Length::Px(64.))
        .height(Length::Px(24.));
    let web = glory::web::launch(
        root,
        view().class("target-view").fill((
            group(),
            text()
                .class("target-text")
                .content("label")
                .style(boxed.clone()),
            button()
                .class("target-button")
                .label("activate")
                .style(boxed.clone()),
            input()
                .class("target-input")
                .value("a🙂z")
                .style(boxed.clone()),
            check()
                .class("target-check")
                .label("check")
                .style(boxed.clone()),
            image()
                .class("target-image")
                .decorative()
                .style(boxed.clone()),
            link()
                .class("target-link")
                .label("link")
                .target("#target")
                .style(boxed),
        )),
    )
    .map_err(js)?;
    let app = web.app();
    let root = app.tree().roots()[0];
    let mut nodes = vec![root];
    nodes.extend(app.tree().node(root).map_err(js)?.children.iter().copied());
    let activations = Rc::new(Cell::new(0));
    for (index, node) in nodes.iter().copied().enumerate() {
        let observed = activations.clone();
        app.scope()
            .on(node, EventKind::Activate, move |_| {
                observed.set(observed.get() + 1)
            })
            .map_err(js)?;
        for kind in [EventKind::FocusWithin, EventKind::Navigate] {
            app.scope().on(node, kind, |_| {}).map_err(js)?;
        }
        for (kind, valid) in [
            (EventKind::Resize, index != 1),
            (EventKind::Scroll, index != 1),
            (EventKind::Anchor, index != 1),
            (EventKind::FocusChanged, index != 1),
            (EventKind::ValueChanged, index == 4),
            (EventKind::Commit, index == 4),
            (EventKind::Composition, index == 4),
            (EventKind::CheckedChanged, index == 5),
        ] {
            let before = app.stats().handlers;
            let accepted = app.scope().on(node, kind, |_| {}).is_ok();
            if accepted != valid || (!valid && app.stats().handlers != before) {
                return Err(js(format!("target interest mismatch: {index}/{kind:?}")));
            }
        }
    }
    app.flush().map_err(js)?;
    DEMO.with(|demo| {
        *demo.borrow_mut() = Some(Demo {
            web,
            nodes,
            responses: Rc::default(),
            activations,
        })
    });
    Ok(())
}
#[wasm_bindgen]
pub fn target_query() -> Result {
    DEMO.with(|demo| {
        let demo = demo.borrow();
        let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
        demo.responses.borrow_mut().clear();
        for (index, node) in demo.nodes.iter().copied().enumerate() {
            for (kind, query) in [
                Query::Bounds(node),
                Query::Scroll(node),
                Query::Selection(node),
                Query::Focus(node),
            ]
            .into_iter()
            .enumerate()
            {
                let responses = demo.responses.clone();
                demo.web
                    .app()
                    .request(query, move |result| {
                        let outcome = match result {
                            Ok(value) if query.validate_response(&value).is_ok() => "ok",
                            Err(Error::Unsupported(_)) => "unsupported",
                            _ => "unexpected",
                        };
                        responses
                            .borrow_mut()
                            .push(format!("{index}:{kind}:{outcome}"));
                    })
                    .map_err(js)?;
            }
        }
        Ok(())
    })
}

#[wasm_bindgen]
pub fn target_focus(index: usize) -> Result {
    DEMO.with(|demo| {
        let demo = demo.borrow();
        let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
        let id = *demo.nodes.get(index).ok_or_else(|| js("index"))?;
        demo.web
            .app()
            .scope()
            .action(glory::render::Action::Focus(id))
            .map_err(js)?;
        demo.web.app().flush().map_err(js)
    })
}

#[wasm_bindgen]
pub fn target_focus_query(index: usize) -> Result {
    use glory::render::QueryValue;
    DEMO.with(|demo| {
        let demo = demo.borrow();
        let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
        let id = *demo.nodes.get(index).ok_or_else(|| js("index"))?;
        demo.responses.borrow_mut().clear();
        let responses = demo.responses.clone();
        demo.web
            .app()
            .request(Query::Focus(id), move |result| {
                let value = match result {
                    Ok(QueryValue::Focus(true)) => "true",
                    Ok(QueryValue::Focus(false)) => "false",
                    Err(Error::Unsupported(_)) => "unsupported",
                    _ => "unexpected",
                };
                responses.borrow_mut().push(value.into());
            })
            .map_err(js)?;
        Ok(())
    })
}

#[wasm_bindgen]
pub fn target_focus_mode(index: usize, explicit: bool) -> Result {
    use glory::render::{Change, Patch};
    DEMO.with(|demo| {
        let demo = demo.borrow();
        let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
        let id = *demo.nodes.get(index).ok_or_else(|| js("index"))?;
        demo.web
            .app()
            .scope()
            .patch(
                id,
                Patch::Focus(Change::Set(if explicit {
                    FocusMode::Programmatic
                } else {
                    FocusMode::Auto
                })),
            )
            .map_err(js)?;
        demo.web.app().flush().map_err(js)
    })
}

#[wasm_bindgen]
pub fn target_enabled(index: usize, enabled: bool) -> Result {
    use glory::render::{Change, Patch};
    DEMO.with(|demo| {
        let demo = demo.borrow();
        let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
        let id = *demo.nodes.get(index).ok_or_else(|| js("index"))?;
        demo.web
            .app()
            .scope()
            .patch(id, Patch::Enabled(Change::Set(enabled)))
            .map_err(js)?;
        demo.web.app().flush().map_err(js)
    })
}

#[wasm_bindgen]
pub fn target_preflight() -> Result<usize> {
    use glory::render::{Change, Descriptor, Patch, Props};
    DEMO.with(|demo| {
        let demo = demo.borrow();
        let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
        let app = demo.web.app();
        let cases = [
            (demo.nodes[0], Patch::Role(Change::Set(Some(Role::Main)))),
            (
                demo.nodes[2],
                Patch::TextKind(Change::Set(TextKind::Paragraph)),
            ),
            (demo.nodes[4], Patch::Multiline(Change::Set(true))),
        ];
        for (id, patch) in &cases {
            let before = app.tree().node(*id).map_err(js)?.descriptor.clone();
            let stats = app.stats();
            if !matches!(
                app.scope().patch(*id, patch.clone()),
                Err(glory::ui::Error::Render(Error::InvalidProps(_)))
            ) {
                return Err(js("structural patch was accepted"));
            }
            if app.stats() != stats || app.tree().node(*id).map_err(js)?.descriptor != before {
                return Err(js("rejected patch changed the semantic model"));
            }
        }
        let stats = app.stats();
        let mut invalid = Descriptor::new(Props::Text(Default::default()));
        invalid
            .options
            .insert(glory::html::HtmlTextOptions::default().tag(glory::html::TextTag::H1));
        if app.scope().create(invalid).is_ok() || app.stats() != stats {
            return Err(js("invalid options allocated a semantic node"));
        }
        let children = app.scope().child_in(demo.nodes[3]).map_err(js)?;
        let group = children.create(Descriptor::new(Props::Group)).map_err(js)?;
        let nested = children.child_in(group).map_err(js)?;
        let stats = app.stats();
        let mut paragraph = Descriptor::new(Props::Text(Default::default()));
        paragraph
            .options
            .insert(glory::html::HtmlTextOptions::default().tag(glory::html::TextTag::P));
        for invalid in [paragraph, Descriptor::new(Props::View)] {
            if !matches!(
                nested.create(invalid),
                Err(glory::ui::Error::Render(Error::InvalidProps(_)))
            ) || app.stats() != stats
            {
                return Err(js(
                    "rejected child allocated identity or changed the staged model/queue",
                ));
            }
        }
        children.dispose();
        // These both retain div and are legal even before the first is committed.
        app.scope()
            .patch(demo.nodes[0], Patch::Role(Change::Set(Some(Role::List))))
            .map_err(js)?;
        app.scope()
            .patch(demo.nodes[0], Patch::Role(Change::Set(Some(Role::Status))))
            .map_err(js)?;
        app.flush().map_err(js)?;
        Ok(cases.len() + 3)
    })
}
#[wasm_bindgen]
pub fn target_stats() -> String {
    DEMO.with(|demo| {
        let demo = demo.borrow();
        let Some(demo) = demo.as_ref() else { return "null".into() };
        let responses: js_sys::Array = demo.responses.borrow().iter().map(JsValue::from).collect();
        let responses = js_sys::JSON::stringify(&responses).unwrap().as_string().unwrap();
        let stats = demo.web.app().stats();
        format!(
            "{{\"nodes\":{},\"handlers\":{},\"queries\":{},\"activations\":{},\"responses\":{responses}}}",
            stats.nodes,
            stats.handlers,
            stats.queries,
            demo.activations.get()
        )
    })
}

#[wasm_bindgen]
pub fn event_probe() -> Result<String> {
    use glory::render::{
        Event, Navigation, NodeIds, Props, Rect, RuntimeId, ScrollAck, ScrollRequest,
    };
    DEMO.with(|demo| {
        let demo = demo.borrow();
        let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
        let app = demo.web.app();
        let root = demo.nodes[0];
        let editor = demo.nodes[4];
        let revision = match app.tree().node(editor).map_err(js)?.descriptor.view().props {
            Props::Input(input) => input.revision,
            _ => return Err(js("missing input")),
        };
        let callbacks = Rc::new(Cell::new(0));
        for (node, kinds) in [
            (
                root,
                vec![
                    EventKind::Scroll,
                    EventKind::Resize,
                    EventKind::Anchor,
                    EventKind::FocusWithin,
                    EventKind::Navigate,
                ],
            ),
            (editor, vec![EventKind::Commit, EventKind::ValueChanged]),
        ] {
            for kind in kinds {
                let count = callbacks.clone();
                app.scope().on(node, kind, move |_| count.set(count.get() + 1)).map_err(js)?;
            }
        }
        let temp = view().mount(app.scope()).map_err(js)?;
        let retired = temp.roots()[0];
        drop(temp);
        let outside = view().mount(app.scope()).map_err(js)?;
        let replacement = outside.roots()[0];
        if retired.slot() != replacement.slot() || retired == replacement {
            return Err(js("slot reuse was not exercised"));
        }
        app.flush().map_err(js)?;
        let foreign = NodeIds::new(RuntimeId::new()).allocate();
        let viewport = Rect::default();
        let cases = vec![
            (
                root,
                Event::Scroll {
                    x: f64::NAN,
                    y: 0.,
                    viewport,
                    ack: None,
                },
            ),
            (
                root,
                Event::Scroll {
                    x: 0.,
                    y: 0.,
                    viewport,
                    ack: Some(ScrollAck {
                        requested: ScrollRequest { x: 0., y: f64::INFINITY },
                        x: 0.,
                        y: 0.,
                    }),
                },
            ),
            (
                root,
                Event::Resize {
                    size: Rect::default(),
                    viewport: Rect { width: -1., ..viewport },
                },
            ),
            (
                root,
                Event::Resize {
                    size: Rect { x: 1., ..viewport },
                    viewport,
                },
            ),
            (
                root,
                Event::Anchor {
                    row: foreign,
                    within: 0.,
                    viewport,
                },
            ),
            (
                root,
                Event::Anchor {
                    row: editor,
                    within: f64::NAN,
                    viewport,
                },
            ),
            (
                root,
                Event::Anchor {
                    row: replacement,
                    within: 0.,
                    viewport,
                },
            ),
            (
                root,
                Event::Anchor {
                    row: root,
                    within: 0.,
                    viewport,
                },
            ),
            (
                root,
                Event::FocusWithin {
                    node: demo.nodes[1],
                    focused: true,
                },
            ),
            (
                root,
                Event::FocusWithin {
                    node: replacement,
                    focused: true,
                },
            ),
            (
                root,
                Event::Navigate {
                    from: editor,
                    command: Navigation::First,
                },
            ),
            (
                editor,
                Event::ValueChanged {
                    value: "bad\nline".into(),
                    composing: false,
                    revision,
                },
            ),
            (
                editor,
                Event::Commit {
                    value: "bad\rline".into(),
                    revision,
                },
            ),
        ];
        let rejected = cases.len();
        for (node, event) in cases {
            if !matches!(
                app.dispatch(node, event),
                Err(glory::ui::Error::Render(Error::InvalidEvent(_) | Error::ForeignRuntime))
            ) {
                return Err(js("malformed event was accepted or misclassified"));
            }
        }
        for event in [
            Event::Anchor {
                row: retired,
                within: 0.,
                viewport,
            },
            Event::FocusWithin {
                node: retired,
                focused: false,
            },
        ] {
            if app.dispatch(root, event).map_err(js)? {
                return Err(js("retired origin reached a handler"));
            }
        }
        if callbacks.get() != 0 {
            return Err(js("malformed event reached a handler"));
        }
        app.dispatch(
            root,
            Event::Scroll {
                x: -3.5,
                y: -1.,
                viewport,
                ack: None,
            },
        )
        .map_err(js)?;
        if callbacks.get() != 1 {
            return Err(js("valid negative offset was rejected"));
        }
        drop(outside);
        app.flush().map_err(js)?;
        let nodes = app.stats().nodes;
        Ok(format!(
            "{{\"rejected\":{rejected},\"malformedCallbacks\":0,\"validCallbacks\":1,\"slotReuse\":true,\"staleIgnored\":true,\"nodes\":{nodes}}}"
        ))
    })
}
