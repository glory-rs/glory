use super::focus_tests::{focus, key, selection, settle};
use super::*;
use glory_core::Cage;
use glory_render::{Change, Patch};
use glory_ui::{check, events, group, input};

#[test]
fn semantic_metadata_updates_preserve_native_editing_and_reset_tab_policy() {
    let mut app = WindowApp::new();
    app.mount(glory_ui::view().name("Panel").fill((
        glory_ui::button().label("before"),
        input().value("a🙂z").name("Editor"),
        check().label("after"),
    )))
    .unwrap();
    let root = app.app.tree().roots()[0];
    let children = app.app.tree().node(root).unwrap().children.clone();
    let editor = children[1];
    let physical = app.renderer.0.borrow().dom.control(editor).unwrap();
    focus(&mut app, editor);
    ime(&mut app, ApaceImeEvent::Preedit("中".into(), Some((3, 3))));
    let before = (selection(&mut app, editor), value(&app, editor));
    for patch in [
        Patch::Name(Change::Set(Some("Renamed editor".into()))),
        Patch::Name(Change::Keep),
        Patch::Name(Change::Clear),
    ] {
        app.app.scope().patch(editor, patch).unwrap();
        app.app.flush().unwrap();
        settle(&mut app);
        assert_eq!((selection(&mut app, editor), value(&app, editor)), before);
        assert_eq!(
            app.renderer.0.borrow().dom.control(editor).unwrap(),
            physical
        );
        assert!(
            app.renderer
                .0
                .borrow()
                .dom
                .range(editor)
                .unwrap()
                .editing
                .composing
        );
    }
    let stats = app.app.stats();
    assert!(
        app.app
            .scope()
            .patch(
                editor,
                Patch::Role(Change::Set(Some(glory_render::Role::List)))
            )
            .is_err()
    );
    assert_eq!(app.app.stats(), stats);
    app.renderer.inspect(|doc| {
        let panel = app.renderer.engine_id(root, Part::Main).unwrap();
        assert_eq!(
            doc.get_node(panel)
                .unwrap()
                .element_data()
                .unwrap()
                .attr(apace_dom::LocalName::from("role")),
            Some("group")
        );
        assert!(
            doc.get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .attr(apace_dom::LocalName::from("aria-label"))
                .is_none()
        );
    });
    ime(&mut app, ApaceImeEvent::Commit("国".into()));
    app.app
        .scope()
        .patch(
            editor,
            Patch::Focus(Change::Set(glory_render::FocusMode::Programmatic)),
        )
        .unwrap();
    app.app.flush().unwrap();
    focus(&mut app, children[0]);
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(
        app.renderer.0.borrow().dom.doc.get_focussed_node_id(),
        Some(app.renderer.0.borrow().dom.control(children[2]).unwrap())
    );
    app.app
        .scope()
        .patch(editor, Patch::Focus(Change::Clear))
        .unwrap();
    app.app.flush().unwrap();
    focus(&mut app, children[0]);
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(
        app.renderer.0.borrow().dom.doc.get_focussed_node_id(),
        Some(physical)
    );
    app.app.clear().unwrap();
}

fn value(app: &WindowApp, node: NodeId) -> String {
    app.renderer.0.borrow().dom.text_value(node).unwrap()
}
fn ime(app: &mut WindowApp, event: ApaceImeEvent) {
    app.handle_ui_event(UiEvent::Ime(event));
    settle(app);
}
fn bound(app: &WindowApp, initial: &str, seen: Rc<RefCell<Vec<Event>>>) -> Cage<String> {
    let value = app.app.scope().cage(initial.to_owned());
    let changes = seen.clone();
    let compositions = seen.clone();
    app.mount(
        input()
            .value(value)
            .on(events::value_changed, move |event| {
                changes.borrow_mut().push(event.clone());
                if let Event::ValueChanged { value: next, .. } = event {
                    value.revise(|value| *value = next);
                }
            })
            .on(events::composition, move |event| {
                compositions.borrow_mut().push(event)
            })
            .on(events::commit, move |event| seen.borrow_mut().push(event)),
    )
    .unwrap();
    value
}

#[test]
fn native_text_echo_preserves_caret_and_enter_commits_without_reentry() {
    let mut app = WindowApp::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let model = bound(&app, "a🙂中z", seen.clone());
    let node = app.app.tree().roots()[0];
    focus(&mut app, node);
    key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
    key(
        &mut app,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::empty(),
    );
    let writes = app.renderer.stats().property_writes;
    key(&mut app, Key::Character("X".into()), Modifiers::empty());
    assert_eq!(model.get(), "a🙂中Xz");
    assert_eq!(selection(&mut app, node), (5, 5));
    assert_eq!(
        app.renderer.stats().property_writes,
        writes,
        "equal echoes must not reset the editor"
    );
    key(&mut app, Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(
        seen.borrow()
            .iter()
            .filter(|event| matches!(event, Event::Commit { value, .. } if value == "a🙂中Xz"))
            .count(),
        1
    );
    assert!(seen.borrow().iter().any(|event| matches!(
        event,
        Event::ValueChanged {
            composing: false,
            revision: 1,
            ..
        }
    )));

    // Queue a native edit, then commit a newer program revision before routing
    // it. Its stale payload must not overwrite the newer controlled value.
    let renderer = app.renderer.clone();
    let physical = renderer.engine_id(node, Part::Main).unwrap();
    EventDriver::new(&mut app, Events::new(renderer)).handle_dom_event(DomEvent::new(
        physical,
        DomEventData::Input(apace_traits::events::ApaceInputEvent {
            value: "obsolete".into(),
        }),
    ));
    model.revise(|value| *value = "server".into());
    app.app.flush().unwrap();
    settle(&mut app);
    assert_eq!(model.get(), "server");
    assert_eq!(value(&app, node), "server");
    assert!(
        !seen
            .borrow()
            .iter()
            .any(|event| matches!(event, Event::ValueChanged { value, .. } if value == "obsolete"))
    );
}

#[test]
fn canonical_multiline_echo_and_explicit_versions_preserve_native_selection_and_composition() {
    let mut app = WindowApp::new();
    let model = app.app.scope().cage("\r\nab\0🙂".to_owned());
    app.mount(
        input()
            .multiline(true)
            .value(model)
            .on(events::value_changed, move |event| {
                if let Event::ValueChanged { value, .. } = event {
                    model.revise(|v| *v = value);
                }
            }),
    )
    .unwrap();
    let node = app.app.tree().roots()[0];
    focus(&mut app, node);
    assert_eq!(value(&app, node), "\nab�🙂");
    {
        let mut state = app.renderer.0.borrow_mut();
        let control = state.dom.control(node).unwrap();
        state
            .dom
            .doc
            .with_text_input(control, |mut driver| driver.select_byte_range(1, 6));
    }
    assert_eq!(selection(&mut app, node), (1, 4));
    let writes = app.renderer.stats().property_writes;
    model.revise(|v| *v = "\nab�🙂".into());
    settle(&mut app);
    assert_eq!(selection(&mut app, node), (1, 4));
    assert_eq!(app.renderer.stats().property_writes, writes);
    app.app
        .scope()
        .patch(
            node,
            Patch::Value {
                value: Change::Set("outside".into()),
                revision: 100,
            },
        )
        .unwrap();
    settle(&mut app);
    key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
    key(&mut app, Key::Character("X".into()), Modifiers::empty());
    assert_eq!(model.get(), "outsideX");
    assert!(
        matches!(&app.app.tree().node(node).unwrap().descriptor.view().props, glory_render::Props::Input(input) if input.revision == 101)
    );
    let raw = "中\0\r\n🙂";
    ime(
        &mut app,
        ApaceImeEvent::Preedit(raw.into(), Some((raw.len(), raw.len()))),
    );
    assert_eq!(value(&app, node), "outsideX中�\n🙂");
    assert_eq!(model.get(), "outsideX中�\n🙂");
    assert_eq!(selection(&mut app, node), (13, 13));
    ime(&mut app, ApaceImeEvent::Commit(raw.into()));
    assert_eq!(model.get(), "outsideX中�\n🙂");
    app.app.clear().unwrap();
}

#[test]
fn email_and_url_purposes_preserve_native_editor_whitespace_and_unicode() {
    for purpose in [glory_render::Purpose::Email, glory_render::Purpose::Url] {
        let mut app = WindowApp::new();
        app.mount(input().purpose(purpose).value("  a\0🙂b  "))
            .unwrap();
        let node = app.app.tree().roots()[0];
        focus(&mut app, node);
        assert_eq!(value(&app, node), "  a�🙂b  ");
        key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
        assert_eq!(selection(&mut app, node), (9, 9));
        app.app.clear().unwrap();
    }
}

#[test]
fn password_purpose_changes_preserve_native_focus_selection_and_composition() {
    let mut app = WindowApp::new();
    let model = bound(&app, "a🙂z", Rc::default());
    let node = app.app.tree().roots()[0];
    focus(&mut app, node);
    key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
    key(
        &mut app,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::empty(),
    );
    ime(&mut app, ApaceImeEvent::Preedit("中".into(), Some((3, 3))));
    let before = (model.get(), value(&app, node), selection(&mut app, node));
    app.app
        .scope()
        .patch(
            node,
            Patch::Purpose(Change::Set(glory_render::Purpose::Password)),
        )
        .unwrap();
    settle(&mut app);
    assert_eq!(
        (model.get(), value(&app, node), selection(&mut app, node)),
        before
    );
    ime(&mut app, ApaceImeEvent::Commit("国".into()));
    assert_eq!(model.get(), "a🙂国z");
    assert_eq!(selection(&mut app, node), (4, 4));
    key(&mut app, Key::Character("!".into()), Modifiers::empty());
    assert_eq!(model.get(), "a🙂国!z");
    app.app.clear().unwrap();
}

#[test]
fn password_pointer_selection_uses_mask_cells_and_typing_reaches_the_raw_unicode_value() {
    use super::tests::pointer;
    let mut app = WindowApp::new();
    app.renderer
        .set_css("body{margin:0}input{box-sizing:border-box;width:200px;height:32px;padding:4px;border:2px solid black;font:18px monospace}");
    let model = app.app.scope().cage("Wi🙂e\u{301}".to_owned());
    app.mount(
        input()
            .purpose(glory_render::Purpose::Password)
            .value(model)
            .on(events::value_changed, move |event| {
                if let Event::ValueChanged { value, .. } = event {
                    model.revise(|v| *v = value);
                }
            }),
    )
    .unwrap();
    let node = app.app.tree().roots()[0];
    focus(&mut app, node);
    let physical = app.renderer.engine_id(node, Part::Main).unwrap();
    app.renderer
        .0
        .borrow_mut()
        .dom
        .doc
        .with_text_input(physical, |mut driver| driver.select_byte_range(2, 2));
    let (x, y) = app.renderer.inspect(|doc| {
        let node = doc.get_node(physical).unwrap();
        let input = node.element_data().unwrap().text_input_data().unwrap();
        let caret = input.cursor_geometry(1.5).unwrap();
        let origin = node.absolute_position(0., 0.);
        let scale = doc.viewport().scale_f64();
        (
            (f64::from(origin.x + node.final_layout().content_box_x()) + caret.x0 / scale + 0.01)
                as f32,
            (f64::from(origin.y + node.final_layout().content_box_y())
                + node.text_input_v_centering_offset(scale)
                + (caret.y0 + caret.y1) / 2. / scale) as f32,
        )
    });
    key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
    app.handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    app.handle_ui_event(UiEvent::PointerUp(pointer(x, y, false)));
    settle(&mut app);
    assert_eq!(selection(&mut app, node), (2, 2));
    key(&mut app, Key::Character("Q".into()), Modifiers::empty());
    assert_eq!(model.get(), "WiQ🙂e\u{301}");
    app.handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    app.handle_ui_event(UiEvent::PointerUp(pointer(x, y, false)));
    settle(&mut app);
    assert_eq!(
        selection(&mut app, node),
        (0, model.get().encode_utf16().count()),
        "password double click selects the masked field"
    );
    key(&mut app, Key::Character("X".into()), Modifiers::empty());
    assert_eq!(model.get(), "X");
    app.app.clear().unwrap();
}

#[test]
fn password_ime_areas_and_scroll_share_masked_geometry_across_values_and_dpi_changes() {
    use apace_traits::shell::ShellProvider;
    use std::sync::{Arc, Mutex};
    #[derive(Default)]
    struct Areas(
        Mutex<Vec<(f32, f32, f32, f32)>>,
        Mutex<Vec<apace_traits::shell::ImePurpose>>,
    );
    impl ShellProvider for Areas {
        fn set_ime_purpose(&self, purpose: apace_traits::shell::ImePurpose) {
            self.1.lock().unwrap().push(purpose);
        }
        fn set_ime_cursor_area(&self, x: f32, y: f32, width: f32, height: f32) {
            self.0.lock().unwrap().push((x, y, width, height));
        }
    }
    let mut results = Vec::new();
    for source in ["WWWWWWWW", "iiiiiiii", "中文中文中文中文"] {
        let mut app = WindowApp::new();
        let areas = Arc::new(Areas::default());
        app.renderer
            .0
            .borrow_mut()
            .dom
            .doc
            .set_shell_provider(areas.clone());
        app.renderer.set_viewport(400, 160, 2.).unwrap();
        app.renderer
            .set_css("body{margin:0}input{box-sizing:border-box;width:240px;height:32px;padding:2px;border:0;font:18px monospace}");
        let model = app.app.scope().cage(source.to_owned());
        app.mount(
            input()
                .purpose(glory_render::Purpose::Password)
                .value(model)
                .on(events::value_changed, move |event| {
                    if let Event::ValueChanged { value, .. } = event {
                        model.revise(|v| *v = value);
                    }
                }),
        )
        .unwrap();
        let node = app.app.tree().roots()[0];
        focus(&mut app, node);
        assert_eq!(
            areas.1.lock().unwrap().last(),
            Some(&apace_traits::shell::ImePurpose::Password)
        );
        key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
        let caret = *areas.0.lock().unwrap().last().unwrap();
        let preedit = "中🙂".repeat(4);
        ime(
            &mut app,
            ApaceImeEvent::Preedit(preedit.clone(), Some((preedit.len(), preedit.len()))),
        );
        assert_eq!(model.get(), format!("{source}{preedit}"));
        let compose = *areas.0.lock().unwrap().last().unwrap();
        assert!(
            compose.2 > caret.2,
            "preedit extent must exceed the caret context: {caret:?}/{compose:?}"
        );
        ime(&mut app, ApaceImeEvent::Commit(preedit));
        let physical = app.renderer.engine_id(node, Part::Main).unwrap();
        app.renderer
            .set_css("body{margin:0}input{box-sizing:border-box;width:48px;height:32px;padding:2px;border:0;font:18px monospace}");
        settle(&mut app);
        let offset = app.renderer.inspect(|doc| {
            doc.get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .text_input_data()
                .unwrap()
                .scroll_offset
        });
        assert!(offset > 0.);
        model.revise(|v| *v = "q".into());
        settle(&mut app);
        let after = app.renderer.inspect(|doc| {
            doc.get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .text_input_data()
                .unwrap()
                .scroll_offset
        });
        assert_eq!(
            after, 0.,
            "a shorter controlled value must retire the old scroll offset"
        );
        results.push((caret, compose, offset));
        app.app
            .scope()
            .patch(node, Patch::Purpose(Change::Clear))
            .unwrap();
        settle(&mut app);
        assert_eq!(
            areas.1.lock().unwrap().last(),
            Some(&apace_traits::shell::ImePurpose::Normal)
        );
        app.app.clear().unwrap();
    }
    assert_eq!(results[0], results[1]);
    assert_eq!(results[1], results[2]);
}

#[test]
fn native_paste_and_ime_normalize_line_endings_without_invalid_single_line_values() {
    let mut app = WindowApp::new();
    let model = bound(&app, "A", Rc::default());
    let node = app.app.tree().roots()[0];
    focus(&mut app, node);
    key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
    ime(&mut app, ApaceImeEvent::Commit("🙂\r\n中".into()));
    assert_eq!(model.get(), "A🙂中");
    assert_eq!(selection(&mut app, node), (4, 4));
    let physical = app.renderer.engine_id(node, Part::Main).unwrap();
    // Reproduce beta.4's clipboard default action, which inserts CR/LF even in
    // a single-line editor, then dispatches Input with that raw buffer.
    app.renderer
        .0
        .borrow_mut()
        .dom
        .doc
        .with_text_input(physical, |mut driver| {
            driver.insert_or_replace_selection("\r\nX")
        });
    let raw = value(&app, node);
    let renderer = app.renderer.clone();
    EventDriver::new(&mut app, Events::new(renderer)).handle_dom_event(DomEvent::new(
        physical,
        DomEventData::Input(apace_traits::events::ApaceInputEvent { value: raw }),
    ));
    settle(&mut app);
    assert_eq!(model.get(), "A🙂中X");
    assert_eq!(value(&app, node), "A🙂中X");
    assert_eq!(selection(&mut app, node), (5, 5));
    ime(
        &mut app,
        ApaceImeEvent::Preedit("b\r\nc".into(), Some((4, 4))),
    );
    assert_eq!(model.get(), "A🙂中Xbc");
    assert_eq!(selection(&mut app, node), (7, 7));
    ime(&mut app, ApaceImeEvent::Commit("\r\n".into()));
    assert_eq!(model.get(), "A🙂中X");

    app.mount(input().multiline(true).value("one\r\ntwo\r"))
        .unwrap();
    let multiline = app.app.tree().roots()[1];
    focus(&mut app, multiline);
    assert_eq!(value(&app, multiline), "one\ntwo\n");
    let physical = app.renderer.engine_id(multiline, Part::Main).unwrap();
    app.renderer
        .0
        .borrow_mut()
        .dom
        .doc
        .with_text_input(physical, |mut driver| driver.move_to_text_end());
    ime(&mut app, ApaceImeEvent::Commit("a\r\nb".into()));
    assert_eq!(value(&app, multiline), "one\ntwo\na\nb");
}

#[test]
fn preedit_echo_empty_end_and_commit_preserve_unicode_and_selected_replacement() {
    let mut app = WindowApp::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let model = bound(&app, "a🙂中z", seen.clone());
    let node = app.app.tree().roots()[0];
    focus(&mut app, node);
    let physical = app.renderer.engine_id(node, Part::Main).unwrap();
    app.renderer
        .0
        .borrow_mut()
        .dom
        .doc
        .with_text_input(physical, |mut driver| driver.select_byte_range(1, 8));
    let writes = app.renderer.stats().property_writes;
    ime(&mut app, ApaceImeEvent::Enabled);
    ime(&mut app, ApaceImeEvent::Preedit("ni".into(), Some((2, 2))));
    assert_eq!(model.get(), "aniz");
    key(
        &mut app,
        Key::Character("duplicate".into()),
        Modifiers::empty(),
    );
    assert_eq!(value(&app, node), "aniz");
    ime(&mut app, ApaceImeEvent::Preedit("你".into(), Some((3, 3))));
    assert_eq!(model.get(), "a你z");
    assert_eq!(selection(&mut app, node), (2, 2));
    assert_eq!(app.renderer.stats().property_writes, writes);
    ime(&mut app, ApaceImeEvent::Preedit(String::new(), None));
    assert_eq!(
        model.get(),
        "a🙂中z",
        "cancel restores the selection replaced by preedit"
    );
    assert_eq!(selection(&mut app, node), (1, 4));
    ime(&mut app, ApaceImeEvent::Commit("你好".into()));
    assert_eq!(model.get(), "a你好z");
    assert_eq!(selection(&mut app, node), (3, 3));
    assert_eq!(app.renderer.stats().property_writes, writes);
    let seen = seen.borrow();
    assert_eq!(
        seen.iter()
            .filter(|event| matches!(event, Event::Composition { active: true }))
            .count(),
        1
    );
    assert_eq!(
        seen.iter()
            .filter(|event| matches!(event, Event::Composition { active: false }))
            .count(),
        1
    );
    assert!(
        matches!(seen.last(), Some(Event::ValueChanged { value, composing: false, .. }) if value == "a你好z")
    );
}

#[test]
fn newer_program_values_win_over_composition_and_trailing_commit() {
    for empty_end in [false, true] {
        let mut app = WindowApp::new();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let model = bound(&app, "base", seen.clone());
        let node = app.app.tree().roots()[0];
        focus(&mut app, node);
        key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
        ime(&mut app, ApaceImeEvent::Preedit("中".into(), Some((3, 3))));
        model.revise(|value| *value = "server".into());
        app.app.flush().unwrap();
        assert_eq!(
            value(&app, node),
            "base中",
            "program writes must not clear live preedit"
        );
        assert_eq!(selection(&mut app, node), (5, 5));
        ime(
            &mut app,
            ApaceImeEvent::Preedit("中国".into(), Some((6, 6))),
        );
        assert_eq!(
            model.get(),
            "server",
            "obsolete input versions must not reach the model"
        );
        if empty_end {
            ime(&mut app, ApaceImeEvent::Preedit(String::new(), None));
            ime(&mut app, ApaceImeEvent::Preedit(String::new(), None));
            assert_eq!(
                value(&app, node),
                "server",
                "cancellation completes without waiting for another key"
            );
        }
        ime(&mut app, ApaceImeEvent::Commit("中国".into()));
        assert_eq!(value(&app, node), "server");
        assert_eq!(model.get(), "server");
        key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
        ime(&mut app, ApaceImeEvent::Commit("!".into()));
        assert_eq!(model.get(), "server!");
        assert_eq!(
            seen.borrow()
                .iter()
                .filter(|event| matches!(event, Event::Composition { active: false }))
                .count(),
            1
        );
    }
}

#[test]
fn keyboard_activation_fires_once_and_space_requires_release_on_the_same_control() {
    use apace_traits::events::KeyState;
    let mut app = WindowApp::new();
    let activated = Rc::new(Cell::new(0));
    let calls = activated.clone();
    let checked = app.app.scope().cage(false);
    app.mount(
        group().fill((
            glory_ui::button()
                .label("button")
                .on(events::activate, move |_| calls.set(calls.get() + 1)),
            check()
                .label("check")
                .checked(checked)
                .on(events::checked_changed, move |event| {
                    if let Event::CheckedChanged(next) = event {
                        checked.revise(|value| *value = next);
                    }
                }),
        )),
    )
    .unwrap();
    let root = app.app.tree().roots()[0];
    let children = app.app.tree().node(root).unwrap().children.clone();
    let release = |app: &mut WindowApp| {
        app.handle_ui_event(UiEvent::KeyUp(ApaceKeyEvent {
            key: Key::Character(" ".into()),
            code: keyboard_types::Code::Space,
            modifiers: Modifiers::empty(),
            location: keyboard_types::Location::Standard,
            is_auto_repeating: false,
            is_composing: false,
            state: KeyState::Released,
            text: None,
        }));
        settle(app);
    };
    focus(&mut app, children[0]);
    key(&mut app, Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(activated.get(), 1);
    key(&mut app, Key::Character(" ".into()), Modifiers::empty());
    key(&mut app, Key::Character(" ".into()), Modifiers::empty());
    assert_eq!(activated.get(), 1);
    release(&mut app);
    release(&mut app);
    assert_eq!(activated.get(), 2);
    key(&mut app, Key::Character(" ".into()), Modifiers::empty());
    focus(&mut app, children[1]);
    release(&mut app);
    assert!(
        !checked.get(),
        "changing focus cancels the armed space activation"
    );
    key(&mut app, Key::Character(" ".into()), Modifiers::empty());
    assert!(!checked.get());
    release(&mut app);
    assert!(checked.get());
}

#[test]
fn ime_surrounding_deletion_keeps_preedit_and_invalid_byte_offsets_do_not_panic() {
    let mut app = WindowApp::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let model = bound(&app, "A🙂中Z", seen);
    let node = app.app.tree().roots()[0];
    focus(&mut app, node);
    key(&mut app, Key::Named(NamedKey::Home), Modifiers::empty());
    key(
        &mut app,
        Key::Named(NamedKey::ArrowRight),
        Modifiers::empty(),
    );
    ime(&mut app, ApaceImeEvent::Preedit("预".into(), Some((3, 3))));
    ime(
        &mut app,
        ApaceImeEvent::DeleteSurrounding {
            before_bytes: 1,
            after_bytes: 4,
        },
    );
    assert_eq!(model.get(), "预中Z");
    assert_eq!(selection(&mut app, node), (1, 1));
    ime(&mut app, ApaceImeEvent::Commit("选".into()));
    assert_eq!(model.get(), "选中Z");
    key(&mut app, Key::Named(NamedKey::Home), Modifiers::empty());
    ime(
        &mut app,
        ApaceImeEvent::Preedit("🙂".into(), Some((1, usize::MAX))),
    );
    ime(
        &mut app,
        ApaceImeEvent::DeleteSurrounding {
            before_bytes: usize::MAX,
            after_bytes: 1,
        },
    );
    assert_eq!(
        value(&app, node),
        "🙂选中Z",
        "a split UTF-8 scalar is not deleted"
    );
    ime(&mut app, ApaceImeEvent::Disabled);
    assert_eq!(model.get(), "选中Z");
}

#[test]
fn multiline_enter_and_suspended_composition_cleanup_use_semantic_lifetimes() {
    let mut app = WindowApp::new();
    let changes = Rc::new(RefCell::new(Vec::new()));
    let output = changes.clone();
    let mounted = input()
        .value("hello")
        .multiline(true)
        .on(events::composition, move |event| {
            output.borrow_mut().push(event)
        })
        .mount(app.app.scope())
        .unwrap();
    let node = app.app.tree().roots()[0];
    focus(&mut app, node);
    key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
    key(&mut app, Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(value(&app, node), "hello\n");
    ime(
        &mut app,
        ApaceImeEvent::Preedit("草稿".into(), Some((6, 6))),
    );
    mounted.suspend().unwrap();
    app.app.flush().unwrap();
    settle(&mut app);
    assert_eq!(
        *changes.borrow(),
        vec![
            Event::Composition { active: true },
            Event::Composition { active: false }
        ]
    );
    assert_eq!(value(&app, node), "hello\n");
    assert!(
        !app.renderer
            .0
            .borrow()
            .dom
            .range(node)
            .unwrap()
            .editing
            .composing
    );
    mounted.place_before(None).unwrap();
    app.app.flush().unwrap();
    focus(&mut app, node);
    ime(&mut app, ApaceImeEvent::Commit("后".into()));
    assert_eq!(value(&app, node), "hello\n后");
    drop(mounted);
    app.app.clear().unwrap();
    settle(&mut app);
    assert!(app.renderer.0.borrow().dom.ranges.is_empty());
}

#[test]
fn checkbox_label_and_control_report_one_typed_change_and_program_updates_work() {
    let mut app = WindowApp::new();
    let checked = app.app.scope().cage(false);
    let changed = Rc::new(RefCell::new(Vec::new()));
    let output = changed.clone();
    app.mount(group().fill(check().label("check").checked(checked).on(
        events::checked_changed,
        move |event| {
            output.borrow_mut().push(event.clone());
            if let Event::CheckedChanged(next) = event {
                checked.revise(|value| *value = next);
            }
        },
    )))
    .unwrap();
    let root = app.app.tree().roots()[0];
    let node = app.app.tree().node(root).unwrap().children[0];
    settle(&mut app);
    let physical = app.renderer.engine_id(node, Part::Control).unwrap();
    let native = |app: &WindowApp| {
        app.renderer.inspect(|doc| {
            doc.get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .checkbox_input_checked()
                .unwrap()
        })
    };
    checked.revise(|value| *value = true);
    settle(&mut app);
    assert!(
        native(&app),
        "beta.4 controlled checked must use its bool representation"
    );
    checked.revise(|value| *value = false);
    settle(&mut app);
    assert!(!native(&app));
    for part in [Part::Main, Part::Control] {
        let target = app.renderer.engine_id(node, part).unwrap();
        let event = app.renderer.inspect(|doc| {
            doc.get_node(target)
                .unwrap()
                .synthetic_click_event_data(Modifiers::empty())
        });
        let renderer = app.renderer.clone();
        EventDriver::new(&mut app, Events::new(renderer))
            .handle_dom_event(DomEvent::new(target, DomEventData::Click(event)));
        settle(&mut app);
    }
    assert_eq!(
        *changed.borrow(),
        vec![Event::CheckedChanged(true), Event::CheckedChanged(false)]
    );
    assert!(!checked.get());
    app.app
        .scope()
        .patch(node, Patch::Enabled(Change::Set(false)))
        .unwrap();
    app.app.flush().unwrap();
    let renderer = app.renderer.clone();
    let event = renderer.inspect(|doc| {
        doc.get_node(physical)
            .unwrap()
            .synthetic_click_event_data(Modifiers::empty())
    });
    EventDriver::new(&mut app, Events::new(renderer))
        .handle_dom_event(DomEvent::new(physical, DomEventData::Click(event)));
    settle(&mut app);
    assert_eq!(changed.borrow().len(), 2);
    assert!(!native(&app));
}

#[test]
fn mixed_check_pointer_label_and_space_clear_only_the_live_mixed_state_once() {
    use glory_render::CheckMode;
    for initially_checked in [false, true] {
        let mut app = WindowApp::new();
        let checked = app.app.scope().cage(initially_checked);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let events = seen.clone();
        app.mount(
            check()
                .mode(CheckMode::Mixed)
                .checked(checked)
                .label("Mixed")
                .on(glory_ui::events::checked_changed, move |event| {
                    events.borrow_mut().push(event.clone());
                    if let Event::CheckedChanged(next) = event {
                        checked.revise(|v| *v = next);
                    }
                }),
        )
        .unwrap();
        let node = app.app.tree().roots()[0];
        settle(&mut app);
        let control = app.renderer.engine_id(node, Part::Control).unwrap();
        let mixed = |app: &WindowApp| {
            app.renderer.inspect(|doc| {
                doc.get_node(control)
                    .unwrap()
                    .element_data()
                    .unwrap()
                    .checkbox_input_indeterminate()
                    .unwrap()
            })
        };
        for (index, part) in [Some(Part::Main), Some(Part::Control), None]
            .into_iter()
            .enumerate()
        {
            // Re-arm with a real property change, like assigning indeterminate
            // again in a browser. Updating checked alone must preserve it.
            for mode in [CheckMode::Checkbox, CheckMode::Mixed] {
                app.app
                    .scope()
                    .patch(node, Patch::CheckMode(Change::Set(mode)))
                    .unwrap();
            }
            checked.revise(|v| *v = initially_checked);
            settle(&mut app);
            assert!(mixed(&app));
            if let Some(part) = part {
                let target = app.renderer.engine_id(node, part).unwrap();
                let event = app.renderer.inspect(|doc| {
                    doc.get_node(target)
                        .unwrap()
                        .synthetic_click_event_data(Modifiers::empty())
                });
                let renderer = app.renderer.clone();
                EventDriver::new(&mut app, Events::new(renderer))
                    .handle_dom_event(DomEvent::new(target, DomEventData::Click(event)));
            } else {
                focus(&mut app, node);
                key(&mut app, Key::Character(" ".into()), Modifiers::empty());
                assert!(mixed(&app), "Space acts on release");
                app.handle_ui_event(UiEvent::KeyUp(ApaceKeyEvent {
                    key: Key::Character(" ".into()),
                    code: keyboard_types::Code::Space,
                    modifiers: Modifiers::empty(),
                    location: keyboard_types::Location::Standard,
                    is_auto_repeating: false,
                    is_composing: false,
                    state: apace_traits::events::KeyState::Released,
                    text: None,
                }));
            }
            settle(&mut app);
            assert!(!mixed(&app));
            assert_eq!(checked.get(), !initially_checked);
            assert_eq!(seen.borrow().len(), index + 1);
            assert_eq!(
                seen.borrow()[index],
                Event::CheckedChanged(!initially_checked)
            );
            assert_eq!(
                app.renderer.engine_id(node, Part::Control).unwrap(),
                control
            );
        }
        app.app
            .scope()
            .patch(node, Patch::CheckMode(Change::Set(CheckMode::Checkbox)))
            .unwrap();
        app.app
            .scope()
            .patch(node, Patch::CheckMode(Change::Set(CheckMode::Mixed)))
            .unwrap();
        app.app
            .scope()
            .patch(node, Patch::Enabled(Change::Set(false)))
            .unwrap();
        settle(&mut app);
        let event = app.renderer.inspect(|doc| {
            doc.get_node(control)
                .unwrap()
                .synthetic_click_event_data(Modifiers::empty())
        });
        let renderer = app.renderer.clone();
        EventDriver::new(&mut app, Events::new(renderer))
            .handle_dom_event(DomEvent::new(control, DomEventData::Click(event)));
        settle(&mut app);
        assert!(mixed(&app));
        assert_eq!(seen.borrow().len(), 3);
        app.app
            .scope()
            .patch(node, Patch::Enabled(Change::Set(true)))
            .unwrap();
        app.app
            .scope()
            .patch(
                node,
                Patch::Semantics(Change::Set(Some(Box::new(glory_render::Semantics {
                    readonly: true,
                    ..Default::default()
                })))),
            )
            .unwrap();
        settle(&mut app);
        let event = app.renderer.inspect(|doc| {
            doc.get_node(control)
                .unwrap()
                .synthetic_click_event_data(Modifiers::empty())
        });
        let renderer = app.renderer.clone();
        EventDriver::new(&mut app, Events::new(renderer))
            .handle_dom_event(DomEvent::new(control, DomEventData::Click(event)));
        settle(&mut app);
        assert!(
            mixed(&app),
            "readonly activation preserves the live mixed state"
        );
        assert_eq!(seen.borrow().len(), 3);
    }
}

#[test]
fn placeholder_updates_leave_selection_preedit_and_empty_pointer_geometry_unchanged() {
    use glory_render::{Purpose, Semantics};
    for purpose in [Purpose::Text, Purpose::Password] {
        let mut app = WindowApp::new();
        app.mount(
            input()
                .purpose(purpose)
                .value("A中Z")
                .semantics(Some(Box::new(Semantics {
                    placeholder: Some("First hint".into()),
                    ..Default::default()
                }))),
        )
        .unwrap();
        let id = app.app.tree().roots()[0];
        let physical = app.renderer.engine_id(id, Part::Main).unwrap();
        focus(&mut app, id);
        ime(&mut app, ApaceImeEvent::Preedit("拼".into(), Some((3, 3))));
        let before = (selection(&mut app, id), value(&app, id));
        let writes = app.renderer.stats().property_writes;
        for change in [
            Change::Set(Some(Box::new(Semantics {
                placeholder: Some("Next hint".into()),
                ..Default::default()
            }))),
            Change::Keep,
            Change::Clear,
        ] {
            let generation = app.renderer.inspect(|doc| {
                doc.get_node(physical)
                    .unwrap()
                    .element_data()
                    .unwrap()
                    .text_input_data()
                    .unwrap()
                    .editor
                    .generation()
            });
            app.app.scope().patch(id, Patch::Semantics(change)).unwrap();
            app.app.flush().unwrap();
            settle(&mut app);
            assert_eq!((selection(&mut app, id), value(&app, id)), before);
            assert_eq!(app.renderer.stats().property_writes, writes);
            app.renderer.inspect(|doc| {
                let data = doc
                    .get_node(physical)
                    .unwrap()
                    .element_data()
                    .unwrap()
                    .text_input_data()
                    .unwrap();
                assert_eq!(data.editor.generation(), generation);
                assert!(data.editor.is_composing());
                assert!(data.placeholder_layout().is_none());
            });
        }
        ime(&mut app, ApaceImeEvent::Preedit("".into(), None));
        app.app
            .scope()
            .patch(
                id,
                Patch::Value {
                    value: Change::Clear,
                    revision: 20,
                },
            )
            .unwrap();
        app.app
            .scope()
            .patch(
                id,
                Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                    placeholder: Some("Long empty hint".into()),
                    ..Default::default()
                })))),
            )
            .unwrap();
        app.app.flush().unwrap();
        settle(&mut app);
        app.renderer
            .0
            .borrow_mut()
            .dom
            .doc
            .with_text_input(physical, |mut d| d.move_to_point(80., 8.));
        assert_eq!(selection(&mut app, id).0, 0);
        assert_eq!(selection(&mut app, id).1, 0);
        assert_eq!(app.renderer.engine_id(id, Part::Main).unwrap(), physical);
    }
}
