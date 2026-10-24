use super::*;
use blitz_traits::events::{BlitzKeyEvent, KeyState};
use glory_render::{Action, FocusMode};
use glory_ui::{Widget, button, check, events, group, input, list, view, widget};
use keyboard_types::{Code, Location};
use std::task::Waker;

pub(super) fn settle(app: &mut WindowApp) {
    app.poll(Some(Context::from_waker(Waker::noop())));
    assert!(app.error.is_none(), "{:?}", app.error);
    assert!(
        !app.renderer.needs_observe(),
        "input/focus geometry needs another host turn"
    );
}
pub(super) fn focus(app: &mut WindowApp, id: NodeId) {
    app.app.scope().action(Action::Focus(id)).unwrap();
    app.app.flush().unwrap();
    settle(app);
}
pub(super) fn key(app: &mut WindowApp, value: Key, modifiers: Modifiers) {
    let text = match &value {
        Key::Character(value) => Some(value.as_str().into()),
        _ => None,
    };
    app.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
        key: value,
        code: Code::Unidentified,
        modifiers,
        location: Location::Standard,
        is_auto_repeating: false,
        is_composing: false,
        state: KeyState::Pressed,
        text,
    }));
    settle(app);
}
pub(super) fn selection(app: &mut WindowApp, id: NodeId) -> (usize, usize) {
    let result = Rc::new(RefCell::new(None));
    let output = result.clone();
    app.app
        .request(Query::Selection(id), move |value| {
            *output.borrow_mut() = Some(value)
        })
        .unwrap();
    settle(app);
    let QueryValue::Selection { start, end } = result.borrow_mut().take().unwrap().unwrap() else {
        panic!("selection")
    };
    (start, end)
}
fn focused(app: &WindowApp) -> Option<NodeId> {
    let state = app.renderer.0.borrow();
    state
        .dom
        .doc
        .get_focussed_node_id()
        .and_then(|id| state.dom.owner(id))
}

#[cfg(feature = "css")]
#[test]
fn portable_focus_styles_follow_the_semantic_check_without_focusing_ancestors() {
    fn bounds(app: &mut WindowApp, node: NodeId) -> glory_render::Rect {
        let answer = Rc::new(RefCell::new(None));
        let result = answer.clone();
        app.app
            .request(Query::Bounds(node), move |value| {
                *result.borrow_mut() = Some(value)
            })
            .unwrap();
        settle(app);
        let QueryValue::Bounds(rect) = answer.borrow_mut().take().unwrap().unwrap() else {
            panic!("bounds")
        };
        rect
    }
    let mut app = WindowApp::new();
    app.renderer.set_sheet(&glory_render::style::Sheet::parse(".outer{width:300px}.outer:focus{width:999px}.choice{display:block;width:70px;height:20px}.choice:focus{width:150px;height:60px}.editor{width:120px}.editor:focus{width:160px}").unwrap()).unwrap();
    app.mount(
        view()
            .class("outer")
            .fill((check().class("choice"), input().class("editor"))),
    )
    .unwrap();
    let outer = app.app.tree().roots()[0];
    let children = app.app.tree().node(outer).unwrap().children.clone();
    let choice = children[0];
    let editor = children[1];
    focus(&mut app, choice);
    assert_eq!(bounds(&mut app, choice).height, 60.0);
    assert_eq!(bounds(&mut app, choice).width, 150.0);
    assert_eq!(bounds(&mut app, outer).width, 300.0);
    focus(&mut app, editor);
    assert_eq!(bounds(&mut app, choice).height, 20.0);
    assert_eq!(bounds(&mut app, editor).width, 160.0);
    assert_eq!(bounds(&mut app, outer).width, 300.0);
    app.app.clear().unwrap();
}

#[test]
fn interactive_target_queries_follow_declared_capabilities() {
    let app = WindowApp::new();
    glory_render::conformance::verify(app.renderer.clone(), app.app.runtime().id(), true);
}
fn has_focus(app: &mut WindowApp, id: NodeId) -> bool {
    let result = Rc::new(RefCell::new(None));
    let output = result.clone();
    app.app
        .request(Query::Focus(id), move |value| {
            *output.borrow_mut() = Some(value)
        })
        .unwrap();
    settle(app);
    let QueryValue::Focus(value) = result.borrow_mut().take().unwrap().unwrap() else {
        panic!("focus query")
    };
    value
}

#[test]
fn rejected_focus_does_not_fault_the_app_or_discard_later_focus_actions() {
    let mut app = WindowApp::new();
    app.renderer.set_css(".hidden{display:none}");
    app.mount(group().fill((
        input().value("first"),
        input().class("hidden"),
        input().value("last"),
    )))
    .unwrap();
    let root = app.app.tree().roots()[0];
    let nodes = app.app.tree().node(root).unwrap().children.clone();
    focus(&mut app, nodes[0]);
    app.app.scope().action(Action::Focus(nodes[1])).unwrap();
    app.app.scope().action(Action::Focus(nodes[2])).unwrap();
    app.app.flush().unwrap();
    app.app
        .flush()
        .expect("an ordinary focus refusal must not fault the App");
    settle(&mut app);
    assert_eq!(
        focused(&app),
        Some(nodes[2]),
        "the later focus intent was discarded"
    );
    key(&mut app, Key::Character("!".into()), Modifiers::empty());
    app.app.clear().unwrap();
}

#[test]
fn focus_queries_read_the_control_at_acceptance_and_distinguish_ancestor_focus() {
    let mut app = WindowApp::new();
    app.mount(
        view()
            .focus(FocusMode::Programmatic)
            .fill((input().value("A"), check().label("B"))),
    )
    .unwrap();
    let root = app.app.tree().roots()[0];
    let nodes = app.app.tree().node(root).unwrap().children.clone();
    focus(&mut app, nodes[1]);
    assert!(
        has_focus(&mut app, nodes[1]),
        "Check queries its inner control"
    );
    assert!(
        !has_focus(&mut app, root),
        "FocusWithin is not exact control focus"
    );
    let mut renderer = app.renderer.clone();
    renderer
        .request(Query::Focus(nodes[1]), QueryToken(987))
        .unwrap();
    renderer.action(Action::Focus(nodes[0])).unwrap();
    let mut pending = Vec::new();
    renderer.drain(&mut pending);
    assert!(
        pending.iter().any(|input| matches!(
            input,
            Input::Response {
                token: QueryToken(987),
                result: Ok(QueryValue::Focus(true)),
                ..
            }
        )),
        "later focus must not rewrite a queued result"
    );
    assert_eq!(focused(&app), Some(nodes[0]));
    app.renderer.set_css("input{display:none}");
    assert!(
        !has_focus(&mut app, nodes[0]),
        "query resolves changed CSS and reconciles focus"
    );
    assert_eq!(focused(&app), None);
    app.renderer.set_css("");
    focus(&mut app, root);
    assert!(has_focus(&mut app, root));
    app.app.clear().unwrap();
}

#[test]
fn focus_disabled_after_queuing_preserves_later_intents_and_edits() {
    let mut app = WindowApp::new();
    app.mount(group().fill((input().value("first"), input().value("last"))))
        .unwrap();
    let root = app.app.tree().roots()[0];
    let nodes = app.app.tree().node(root).unwrap().children.clone();
    app.app.scope().action(Action::Focus(nodes[0])).unwrap();
    app.app
        .scope()
        .patch(
            nodes[0],
            glory_render::Patch::Enabled(glory_render::Change::Set(false)),
        )
        .unwrap();
    app.app.scope().action(Action::Focus(nodes[1])).unwrap();
    app.app.flush().unwrap();
    settle(&mut app);
    assert!(!has_focus(&mut app, nodes[0]));
    assert!(has_focus(&mut app, nodes[1]));
    app.app
        .scope()
        .patch(
            nodes[0],
            glory_render::Patch::Enabled(glory_render::Change::Clear),
        )
        .unwrap();
    focus(&mut app, nodes[0]);
    assert!(has_focus(&mut app, nodes[0]));
    app.app.clear().unwrap();
}

#[test]
fn focus_transitions_cross_groups_in_order_and_control_parts_are_not_lost() {
    let mut app = WindowApp::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let events = seen.clone();
    app.mount(group().fill((
        input().value("A"),
        check().label("B"),
        view().focus(FocusMode::Programmatic),
    )))
    .unwrap();
    let root = app.app.tree().roots()[0];
    app.app
        .scope()
        .on(root, events::focus_within, move |event| {
            events.borrow_mut().push(event)
        })
        .unwrap();
    let children = app.app.tree().node(root).unwrap().children.clone();
    assert!(app.app.scope().capabilities().unwrap().focus);
    assert!(
        app.app.scope().capabilities().unwrap().input,
        "WindowApp installs the semantic editor protocol"
    );
    // No intermediate pump: A/B/A must remain ordered in the adapter queue.
    for id in [children[0], children[1], children[0]] {
        app.app.scope().action(Action::Focus(id)).unwrap();
        app.app.flush().unwrap();
    }
    settle(&mut app);
    assert_eq!(
        seen.borrow().as_slice(),
        &[
            Event::FocusWithin {
                node: children[0],
                focused: true
            },
            Event::FocusWithin {
                node: children[0],
                focused: false
            },
            Event::FocusWithin {
                node: children[1],
                focused: true
            },
            Event::FocusWithin {
                node: children[1],
                focused: false
            },
            Event::FocusWithin {
                node: children[0],
                focused: true
            },
        ]
    );
    let expected = app.renderer.engine_id(children[1], Part::Control).unwrap();
    focus(&mut app, children[1]);
    assert_eq!(
        app.renderer.inspect(|doc| doc.get_focussed_node_id()),
        Some(expected)
    );
    focus(&mut app, children[2]);
    assert_eq!(focused(&app), Some(children[2]));
    app.app.clear().unwrap();
    settle(&mut app);
    assert_eq!(focused(&app), None);
    assert!(app.renderer.0.borrow().dom.controls.is_empty());
    assert!(app.renderer.0.borrow().subscriptions.is_empty());
}

#[test]
fn tab_starts_from_no_focus_skips_disabled_and_programmatic_nodes_and_native_editors_keep_caret_keys()
 {
    let mut app = WindowApp::new();
    app.renderer.set_css(".hidden{display:none}");
    app.mount(group().fill((
        button().label("first"),
        button().label("disabled").enabled(false),
        view().focus(FocusMode::Programmatic),
        input().value("a🙂中z"),
        check().label("last"),
        view().class("hidden").fill(input().value("hidden")),
    )))
    .unwrap();
    let root = app.app.tree().roots()[0];
    let children = app.app.tree().node(root).unwrap().children.clone();
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), Some(children[0]));
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), Some(children[3]));
    key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
    assert_eq!(selection(&mut app, children[3]), (5, 5));
    key(
        &mut app,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::empty(),
    );
    assert_eq!(selection(&mut app, children[3]), (4, 4));
    key(
        &mut app,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::empty(),
    );
    assert_eq!(selection(&mut app, children[3]), (3, 3));
    key(
        &mut app,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::empty(),
    );
    assert_eq!(selection(&mut app, children[3]), (1, 1));
    key(&mut app, Key::Named(NamedKey::Home), Modifiers::empty());
    assert_eq!(selection(&mut app, children[3]), (0, 0));
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), Some(children[4]));
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::CONTROL);
    assert_eq!(focused(&app), Some(children[4]));
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), Some(children[0]));
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::SHIFT);
    assert_eq!(focused(&app), Some(children[4]));
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::SHIFT);
    assert_eq!(focused(&app), Some(children[3]));
    focus(&mut app, children[2]);
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), Some(children[3]));
    app.app
        .scope()
        .patch(
            children[3],
            glory_render::Patch::Enabled(glory_render::Change::Set(false)),
        )
        .unwrap();
    app.app.flush().unwrap();
    settle(&mut app);
    assert_eq!(focused(&app), None);
}

struct Live(Rc<Cell<usize>>);
impl Live {
    fn new(count: Rc<Cell<usize>>) -> Self {
        count.set(count.get() + 1);
        Self(count)
    }
}

#[test]
fn computed_visibility_and_hidden_ancestors_reject_focus_but_visible_descendants_can_override_visibility()
 {
    let mut app = WindowApp::new();
    app.renderer
        .set_css(".ghost{visibility:hidden}.restored{visibility:visible}");
    app.mount(
        group().fill((
            view()
                .class("ghost")
                .fill(input().class("restored").value("visible")),
            input().class("ghost").value("hidden"),
        )),
    )
    .unwrap();
    let root = app.app.tree().roots()[0];
    let nodes = app.app.tree().node(root).unwrap().children.clone();
    let visible = app.app.tree().node(nodes[0]).unwrap().children[0];
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), Some(visible));
    assert!(matches!(
        app.renderer.clone().action(Action::Focus(nodes[1])),
        Err(Error::Unsupported(_))
    ));
    app.renderer.set_css(".ghost{visibility:hidden}");
    settle(&mut app);
    assert_eq!(focused(&app), None);
    app.renderer
        .set_css(".ghost{display:none}.restored{display:block;visibility:visible}");
    settle(&mut app);
    assert!(matches!(
        app.renderer.clone().action(Action::Focus(visible)),
        Err(Error::Unsupported(_))
    ));
}
impl Drop for Live {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

#[test]
fn huge_fixed_and_measured_lists_pin_native_editors_and_navigate_without_source_scans() {
    for (adaptive, password) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut app = WindowApp::new();
        app.renderer
            .set_css("body{font:14px sans-serif}input{box-sizing:border-box;padding:0;border:0}");
        let position = app.app.scope().cage(0.0);
        let base = app.app.scope().cage(100_usize);
        let live = Rc::new(Cell::new(0));
        let rows_live = live.clone();
        let reads = Rc::new(Cell::new(0));
        let source_reads = reads.clone();
        let latest = Rc::new(RefCell::new(None));
        let observed = latest.clone();
        let widget = list(
            100_000_000_usize,
            move |index| {
                source_reads.set(source_reads.get() + 1);
                index + base.get()
            },
            move |_index, key| {
                let live = rows_live.clone();
                widget(move |scope| {
                    let guard = Live::new(live.clone());
                    scope.owner().spawn(async move {
                        let _guard = guard;
                        std::future::pending::<()>().await;
                    })?;
                    let value = scope.cage(format!("draft🙂:{key}"));
                    input()
                        .purpose(if password {
                            glory_render::Purpose::Password
                        } else {
                            glory_render::Purpose::Text
                        })
                        .value(value)
                        .on(events::value_changed, move |event| {
                            if let Event::ValueChanged { value: next, .. } = event {
                                value.revise(|value| *value = next);
                            }
                        })
                        .style(glory_render::style::Style::new().height(
                            glory_render::style::Length::Px(if adaptive {
                                [24., 28., 32.][key % 3]
                            } else {
                                24.
                            }),
                        ))
                        .show_in(scope)
                })
            },
        )
        .height(180.)
        .overscan(2)
        .row_limit(20)
        .position(position)
        .locate(move |key| {
            key.checked_sub(base.get())
                .filter(|index| *index < 100_000_000)
        })
        .inspect(move |stats| *observed.borrow_mut() = Some(stats));
        let mounted = (if adaptive {
            widget.measure(24.).measure_limit(64)
        } else {
            widget.row_height(24.)
        })
        .mount(app.app.scope())
        .unwrap();
        app.app.flush().unwrap();
        app.mount(button().label("outside")).unwrap();
        settle(&mut app);
        let root = app.app.tree().roots()[0];
        let outside = app.app.tree().roots()[1];
        let surface = app.app.tree().node(root).unwrap().children[0];
        let row = app.app.tree().node(surface).unwrap().children[3];
        let editor = app.app.tree().node(row).unwrap().children[0];
        let physical = app.renderer.engine_id(editor, Part::Main).unwrap();
        focus(&mut app, editor);
        key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
        // Engine-only typing exercises preservation; value/composition routing
        // remains intentionally unadvertised until its revision contract lands.
        key(&mut app, Key::Character("X".into()), Modifiers::empty());
        app.handle_ui_event(UiEvent::Ime(BlitzImeEvent::Preedit(
            "中".into(),
            Some((3, 3)),
        )));
        settle(&mut app);
        let value = app.renderer.inspect(|doc| {
            doc.get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .text_input_data()
                .unwrap()
                .editor
                .raw_text()
                .to_string()
        });
        let caret = selection(&mut app, editor);
        assert!(
            value.ends_with("X中"),
            "controlled composition draft not inserted: {value}"
        );
        for step in 1..=30 {
            position.revise(|position| *position = step as f64 * 71_923_000.);
            settle(&mut app);
            assert_eq!(focused(&app), Some(editor));
            assert_eq!(
                app.renderer.engine_id(editor, Part::Main).unwrap(),
                physical
            );
            assert_eq!(selection(&mut app, editor), caret);
            let reads_before = reads.get();
            assert!(has_focus(&mut app, editor));
            assert_eq!(
                reads.get(),
                reads_before,
                "focus queries must not read list source keys"
            );
            assert_eq!(latest.borrow().as_ref().unwrap().pinned, 1);
            assert!(live.get() <= 15);
            assert!(app.app.runtime_stats().node_slots < 256);
        }
        assert!(app.renderer.inspect(|doc| {
            doc.get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .text_input_data()
                .unwrap()
                .editor
                .raw_text()
                == value
        }));
        assert!(
            matches!(&app.app.tree().node(editor).unwrap().descriptor.view().props, glory_render::Props::Input(input) if input.value == value)
        );
        let before = app
            .app
            .tree()
            .node(row)
            .unwrap()
            .descriptor
            .view()
            .item
            .unwrap()
            .index;
        base.revise(|base| *base -= 1);
        settle(&mut app);
        assert_eq!(focused(&app), Some(editor));
        assert_eq!(selection(&mut app, editor), caret);
        assert_eq!(
            app.app
                .tree()
                .node(row)
                .unwrap()
                .descriptor
                .view()
                .item
                .unwrap()
                .index,
            before + 1
        );
        mounted.suspend().unwrap();
        app.app.flush().unwrap();
        settle(&mut app);
        assert_eq!(focused(&app), None);
        mounted.place_before(None).unwrap();
        app.app.flush().unwrap();
        settle(&mut app);
        assert_eq!(
            latest.borrow().as_ref().unwrap().pinned,
            0,
            "suspended focus loss must release the old pin"
        );
        focus(&mut app, outside);
        assert_eq!(latest.borrow().as_ref().unwrap().pinned, 0);
        assert!(app.renderer.inspect(|doc| doc.get_node(physical).is_none()));
        focus(&mut app, root);
        key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
        let end = focused(&app).unwrap();
        assert_eq!(
            app.app
                .tree()
                .node(end)
                .unwrap()
                .descriptor
                .view()
                .item
                .unwrap()
                .index,
            99_999_999
        );
        let before = reads.get();
        key(&mut app, Key::Named(NamedKey::Home), Modifiers::CONTROL);
        assert_eq!(focused(&app), Some(end));
        assert_eq!(reads.get(), before);
        key(&mut app, Key::Named(NamedKey::Home), Modifiers::empty());
        let first = focused(&app).unwrap();
        assert_eq!(
            app.app
                .tree()
                .node(first)
                .unwrap()
                .descriptor
                .view()
                .item
                .unwrap()
                .index,
            0
        );
        let first_editor = app.app.tree().node(first).unwrap().children[0];
        focus(&mut app, first_editor);
        base.revise(|base| *base += 1);
        settle(&mut app);
        assert_eq!(
            focused(&app),
            Some(root),
            "focused key deletion returns focus to collection"
        );
        assert!(reads.get() < 2500, "adaptive={adaptive}: {}", reads.get());
        drop(mounted);
        app.app.clear().unwrap();
        settle(&mut app);
        assert_eq!(live.get(), 0);
        assert_eq!(focused(&app), None);
        assert_eq!(app.renderer.stats().nodes, 0);
    }
}
