use super::tests::{bounds_now, native_focus, pointer, tick};
use super::*;
use glory_render::{DragPhase, KeyboardCommand};
use glory_ui::{button, input, view};

fn key(app: &mut WindowApp, key: &str, modifiers: Modifiers, composing: bool, repeat: bool) {
    app.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
        key: key.parse().unwrap(),
        code: keyboard_types::Code::Unidentified,
        modifiers,
        location: keyboard_types::Location::Standard,
        is_auto_repeating: repeat,
        is_composing: composing,
        state: blitz_traits::events::KeyState::Pressed,
        text: None,
    }));
}

#[test]
fn native_grid_navigation_is_queued_two_dimensional_and_preserves_nested_editor_and_modified_keys()
{
    use super::focus_tests::{focus, settle};
    use glory_render::{GridCommand, Semantics};
    let mut host = WindowApp::new();
    let day = host.app.scope().cage(None);
    let editor = host.app.scope().cage(None);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let out = calls.clone();
    host.mount(
        view()
            .role(glory_render::Role::Grid)
            .name("February 2024")
            .semantics(Some(Box::new(Semantics {
                grid_navigation: Some(glory_render::Direction::Ltr),
                ..Default::default()
            })))
            .on(EventKind::GridNavigate, move |event| {
                if let Event::GridNavigate { command, .. } = event {
                    out.borrow_mut().push(command);
                }
            })
            .fill((
                glory_ui::group().fill(button().label("29").node_ref(day)),
                input().value("12").node_ref(editor),
            )),
    )
    .unwrap();
    let id = day.get().unwrap();
    focus(&mut host, id);
    key(&mut host, "ArrowDown", Modifiers::empty(), false, false);
    assert!(calls.borrow().is_empty());
    tick(&mut host);
    assert_eq!(calls.borrow().as_slice(), [GridCommand::Down]);
    key(&mut host, "PageUp", Modifiers::SHIFT, false, false);
    tick(&mut host);
    assert_eq!(calls.borrow().last(), Some(&GridCommand::PreviousLargePage));
    for modifiers in [Modifiers::CONTROL, Modifiers::ALT, Modifiers::META] {
        key(&mut host, "ArrowLeft", modifiers, false, false);
        tick(&mut host);
    }
    key(&mut host, "ArrowRight", Modifiers::empty(), true, false);
    tick(&mut host);
    assert_eq!(calls.borrow().len(), 2);
    focus(&mut host, editor.get().unwrap());
    key(&mut host, "ArrowLeft", Modifiers::empty(), false, false);
    settle(&mut host);
    assert_eq!(calls.borrow().len(), 2);
    host.app.clear().unwrap();
    assert!(day.get().is_none());
}

#[test]
fn native_opt_in_edit_captures_current_selection_intent_and_ime_without_the_legacy_value_path() {
    use super::focus_tests::{focus, key as editor_key, settle};
    use glory_render::{
        InputEdit, InputEditCapture, InputEditIntent, InputEditRejection, TextSelection,
    };
    let mut app = WindowApp::new();
    let r = app.app.scope().cage(None);
    let edits = Rc::new(RefCell::new(Vec::new()));
    let out = edits.clone();
    let legacy = Rc::new(Cell::new(0));
    let seen = legacy.clone();
    let commits = Rc::new(Cell::new(0));
    let committed = commits.clone();
    app.mount(
        input()
            .value("12-34")
            .node_ref(r)
            .semantics(Some(Box::new(glory_render::Semantics {
                input_edit: Some(InputEditCapture { max_bytes: 8 }),
                ..Default::default()
            })))
            .on(EventKind::InputEdit, move |event| {
                if let Event::InputEdit(edit) = event {
                    out.borrow_mut().push(edit);
                }
            })
            .on(EventKind::ValueChanged, move |_| seen.set(seen.get() + 1))
            .on(EventKind::Commit, move |_| {
                committed.set(committed.get() + 1)
            }),
    )
    .unwrap();
    let id = r.get().unwrap();
    focus(&mut app, id);
    app.app
        .scope()
        .set_input_selection(
            id,
            TextSelection {
                start: 3,
                end: 3,
                ..Default::default()
            },
        )
        .unwrap();
    app.app.flush().unwrap();
    settle(&mut app);
    editor_key(
        &mut app,
        Key::Named(NamedKey::Backspace),
        Modifiers::empty(),
    );
    assert_eq!(legacy.get(), 0);
    assert_eq!(edits.borrow().len(), 1);
    assert!(
        matches!(&edits.borrow()[0],InputEdit::Value{intent:InputEditIntent::DeleteBackward,before:Some(TextSelection{start:3,end:3,..}),selection:TextSelection{start:2,end:2,..},composing:false,value,..} if value.len()==4)
    );
    editor_key(&mut app, Key::Character("5".into()), Modifiers::empty());
    assert!(matches!(
        edits.borrow().last(),
        Some(InputEdit::Value {
            intent: InputEditIntent::Insert,
            before: Some(TextSelection { start: 2, .. }),
            selection: TextSelection { start: 3, .. },
            ..
        })
    ));
    editor_key(&mut app, Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(commits.get(), 1);
    assert_eq!(legacy.get(), 0);
    app.handle_ui_event(UiEvent::Ime(BlitzImeEvent::Preedit(
        "xx".into(),
        Some((2, 2)),
    )));
    settle(&mut app);
    assert!(matches!(
        edits.borrow().last(),
        Some(InputEdit::Value {
            composing: true,
            ..
        })
    ));
    app.handle_ui_event(UiEvent::Ime(BlitzImeEvent::Commit("000".into())));
    settle(&mut app);
    assert!(
        matches!(edits.borrow().last(),Some(InputEdit::Value{composing:false,value,..})if value.len()==8)
    );
    assert_eq!(legacy.get(), 0);
    let physical = app.renderer.engine_id(id, Part::Main).unwrap();
    let renderer = app.renderer.clone();
    EventDriver::new(&mut app, Events(renderer)).handle_dom_event(DomEvent::new(
        physical,
        DomEventData::Input(blitz_traits::events::BlitzInputEvent {
            value: "0".repeat(1_000_000),
        }),
    ));
    settle(&mut app);
    assert!(matches!(
        edits.borrow().last(),
        Some(InputEdit::Rejected {
            reason: InputEditRejection::TooLong,
            ..
        })
    ));
    assert_eq!(legacy.get(), 0);
}

#[test]
fn native_select_text_preserves_direction_and_revision_and_explicitly_refuses_unicode_boundaries() {
    use super::focus_tests::{focus, selection, settle};
    use glory_render::{Action, SelectionDirection, TextSelection};
    for masked in [false, true] {
        let mut app = WindowApp::new();
        let r = app.app.scope().cage(None);
        app.mount(
            input()
                .value("000001")
                .purpose(if masked {
                    glory_render::Purpose::Password
                } else {
                    glory_render::Purpose::Text
                })
                .node_ref(r),
        )
        .unwrap();
        let id = r.get().unwrap();
        focus(&mut app, id);
        app.app
            .scope()
            .set_input_selection(
                id,
                TextSelection {
                    start: 1,
                    end: 4,
                    direction: SelectionDirection::Backward,
                },
            )
            .unwrap();
        app.app.flush().unwrap();
        settle(&mut app);
        assert_eq!(selection(&mut app, id), (1, 4));
        let physical = app.renderer.engine_id(id, Part::Main).unwrap();
        app.renderer.inspect(|doc| {
            let editor = &doc
                .get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .text_input_data()
                .unwrap()
                .editor;
            assert_eq!(editor.raw_selection().anchor().index(), 4);
            assert_eq!(editor.raw_selection().focus().index(), 1);
        });
        let stale = Action::SelectText {
            node: id,
            revision: 0,
            selection: TextSelection::default(),
        };
        app.app.scope().set_input_value(id, "000002").unwrap();
        app.app
            .scope()
            .set_input_selection(
                id,
                TextSelection {
                    start: 2,
                    end: 2,
                    direction: SelectionDirection::None,
                },
            )
            .unwrap();
        app.app.flush().unwrap();
        settle(&mut app);
        assert_eq!(selection(&mut app, id), (2, 2));
        let before = app.renderer.stats();
        assert!(app.renderer.clone().action(stale).is_err());
        assert_eq!(app.renderer.stats(), before);
        app.app.scope().set_input_value(id, "e\u{301}").unwrap();
        app.app.flush().unwrap();
        let before = app.app.stats();
        assert!(
            app.app
                .scope()
                .set_input_selection(
                    id,
                    TextSelection {
                        start: 1,
                        end: 1,
                        ..Default::default()
                    }
                )
                .is_err()
        );
        assert_eq!(app.app.stats(), before);
    }
}

#[test]
fn native_ch_input_assist_preserves_real_pointer_caret_keyboard_editing_and_whole_replacement() {
    use super::focus_tests::{focus, key as editor_key, selection, settle};
    use glory_render::style::{Edges, Family, Length, Style};
    use glory_render::{InputAssist, InputAutofill, InputMode, Purpose, Semantics};
    for masked in [false, true] {
        let mut app = WindowApp::new();
        let model = app.app.scope().cage("000000".to_owned());
        let reference = app.app.scope().cage(None);
        app.mount(
            input()
                .value(model)
                .purpose(if masked {
                    Purpose::Password
                } else {
                    Purpose::Text
                })
                .node_ref(reference)
                .style(
                    Style {
                        font_size: Some(24.),
                        ..Style::new()
                    }
                    .font_family([Family::Mono])
                    .width(Length::Ch(6.))
                    .height(Length::Px(48.))
                    .padding(Edges::all(0.))
                    .border_width(Edges::all(0.)),
                )
                .semantics(Some(Box::new(Semantics {
                    input_assist: Some(InputAssist {
                        autofill: Some(InputAutofill::OneTimeCode),
                        mode: Some(InputMode::Numeric),
                    }),
                    ..Default::default()
                })))
                .on(EventKind::ValueChanged, move |e| {
                    if let Event::ValueChanged { value, .. } = e {
                        model.revise(|v| *v = value);
                    }
                }),
        )
        .unwrap();
        let id = reference.get().unwrap();
        focus(&mut app, id);
        let rect = bounds_now(&app, id);
        let physical = app.renderer.engine_id(id, Part::Main).unwrap();
        editor_key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
        let x = (rect.x + rect.width * 2.01 / 6.) as f32;
        let y = (rect.y + rect.height / 2.) as f32;
        app.handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
        app.handle_ui_event(UiEvent::PointerUp(pointer(x, y, false)));
        settle(&mut app);
        assert_eq!(selection(&mut app, id), (2, 2));
        editor_key(
            &mut app,
            Key::Named(NamedKey::Backspace),
            Modifiers::empty(),
        );
        assert_eq!(model.get().len(), 5);
        assert_eq!(selection(&mut app, id), (1, 1));
        editor_key(&mut app, Key::Named(NamedKey::Home), Modifiers::empty());
        editor_key(
            &mut app,
            Key::Named(NamedKey::ArrowRight),
            Modifiers::empty(),
        );
        assert_eq!(selection(&mut app, id), (1, 1));
        // Explicitly exercise the editor operation and native Input dispatch
        // used after a host clipboard insertion. This is not an OS clipboard claim.
        app.renderer
            .0
            .borrow_mut()
            .dom
            .doc
            .with_text_input(physical, |mut driver| {
                driver.select_byte_range(0, 5);
                driver.insert_or_replace_selection("000001");
            });
        let raw = app.renderer.0.borrow().dom.text_value(id).unwrap();
        let renderer = app.renderer.clone();
        EventDriver::new(&mut app, Events(renderer)).handle_dom_event(DomEvent::new(
            physical,
            DomEventData::Input(blitz_traits::events::BlitzInputEvent { value: raw }),
        ));
        settle(&mut app);
        assert_eq!(model.get().len(), 6);
        assert_eq!(selection(&mut app, id), (6, 6));
        assert_eq!(app.renderer.engine_id(id, Part::Main).unwrap(), physical);
    }
}

#[test]
fn native_form_requests_queue_enter_and_button_defaults_without_resetting_the_editor() {
    use glory_render::{FieldPart, FormAction, Semantics};
    let mut app = WindowApp::new();
    let input_ref = app.app.scope().cage(None);
    let submit_ref = app.app.scope().cage(None);
    let reset_ref = app.app.scope().cage(None);
    let events = Rc::new(RefCell::new(Vec::new()));
    let submit_seen = events.clone();
    let reset_seen = events.clone();
    app.mount(
        view()
            .field_part(FieldPart::Form)
            .style(glory_render::style::Style::new().width(glory_render::style::Length::Px(360.0)))
            .on(EventKind::Submit, move |event| {
                submit_seen.borrow_mut().push(event)
            })
            .on(EventKind::Reset, move |event| {
                reset_seen.borrow_mut().push(event)
            })
            .fill((
                input().value("preserved").node_ref(input_ref),
                glory_ui::button()
                    .label("Submit")
                    .node_ref(submit_ref)
                    .semantics(Some(Box::new(Semantics {
                        form_action: Some(FormAction::Submit),
                        ..Default::default()
                    }))),
                glory_ui::button()
                    .label("Reset")
                    .node_ref(reset_ref)
                    .semantics(Some(Box::new(Semantics {
                        form_action: Some(FormAction::Reset),
                        ..Default::default()
                    }))),
            )),
    )
    .unwrap();
    tick(&mut app);
    let input = input_ref.get().unwrap();
    let physical = app
        .renderer
        .engine_id(input, glory_html::Part::Main)
        .unwrap();
    native_focus(&mut app, input);
    key(&mut app, "Enter", Modifiers::empty(), false, false);
    assert!(events.borrow().is_empty(), "native callbacks only enqueue");
    tick(&mut app);
    assert_eq!(
        events.borrow().as_slice(),
        [Event::Submit { submitter: None }]
    );
    for (button, expected) in [
        (
            submit_ref.get().unwrap(),
            Event::Submit {
                submitter: submit_ref.get(),
            },
        ),
        (reset_ref.get().unwrap(), Event::Reset),
    ] {
        let rect = bounds_now(&app, button);
        app.handle_ui_event(UiEvent::PointerMove(pointer(
            (rect.x + 8.) as f32,
            (rect.y + 8.) as f32,
            false,
        )));
        app.handle_ui_event(UiEvent::PointerDown(pointer(
            (rect.x + 8.) as f32,
            (rect.y + 8.) as f32,
            true,
        )));
        app.handle_ui_event(UiEvent::PointerUp(pointer(
            (rect.x + 8.) as f32,
            (rect.y + 8.) as f32,
            false,
        )));
        tick(&mut app);
        assert_eq!(events.borrow().last(), Some(&expected));
    }
    assert_eq!(
        app.renderer
            .engine_id(input, glory_html::Part::Main)
            .unwrap(),
        physical
    );
    app.renderer.inspect(|doc| {
        assert_eq!(
            doc.get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .text_input_data()
                .unwrap()
                .editor
                .raw_text(),
            "preserved"
        )
    });
}

#[test]
fn native_field_label_readonly_and_disabled_group_preserve_editor_identity() {
    use glory_render::style::{Length, Style};
    use glory_render::{FieldPart, SemanticId, Semantics};
    let mut app = WindowApp::new();
    let editor = app.app.scope().cage(None);
    let label = app.app.scope().cage(None);
    let group = app.app.scope().cage(None);
    let name = SemanticId::new("native-field-editor").unwrap();
    app.mount(
        view()
            .field_part(FieldPart::Fieldset)
            .node_ref(group)
            .style(Style::new().width(Length::Px(360.0)))
            .fill((
                view()
                    .field_part(FieldPart::Legend)
                    .fill(glory_ui::text().content("Preferences")),
                view()
                    .field_part(FieldPart::Label)
                    .node_ref(label)
                    .style(
                        Style::new()
                            .width(Length::Px(240.0))
                            .height(Length::Px(32.0)),
                    )
                    .semantics(Some(Box::new(Semantics {
                        field_part: Some(FieldPart::Label),
                        label_for: Some(name.clone()),
                        ..Default::default()
                    })))
                    .fill(glory_ui::text().content("Native field label")),
                input()
                    .node_ref(editor)
                    .value("draft")
                    .style(
                        Style::new()
                            .width(Length::Px(240.0))
                            .height(Length::Px(36.0)),
                    )
                    .semantics(Some(Box::new(Semantics {
                        id: Some(name.clone()),
                        readonly: true,
                        required: true,
                        invalid: true,
                        ..Default::default()
                    }))),
            )),
    )
    .unwrap();
    tick(&mut app);
    let id = editor.get().unwrap();
    let physical = app.renderer.engine_id(id, glory_html::Part::Main).unwrap();
    let rect = bounds_now(&app, label.get().unwrap());
    app.handle_ui_event(UiEvent::PointerMove(pointer(
        (rect.x + 8.) as f32,
        (rect.y + 12.) as f32,
        false,
    )));
    app.handle_ui_event(UiEvent::PointerDown(pointer(
        (rect.x + 8.) as f32,
        (rect.y + 12.) as f32,
        true,
    )));
    app.handle_ui_event(UiEvent::PointerUp(pointer(
        (rect.x + 8.) as f32,
        (rect.y + 12.) as f32,
        false,
    )));
    tick(&mut app);
    assert_eq!(
        app.renderer.0.borrow().focused,
        Some(id),
        "external native label must focus its target"
    );
    key(&mut app, "x", Modifiers::empty(), false, false);
    key(&mut app, "Backspace", Modifiers::empty(), false, false);
    tick(&mut app);
    app.renderer.inspect(|doc| {
        assert_eq!(
            doc.get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .text_input_data()
                .unwrap()
                .editor
                .raw_text(),
            "draft"
        )
    });
    let field_state = |readonly| {
        glory_render::Patch::Semantics(glory_render::Change::Set(Some(Box::new(Semantics {
            id: Some(name.clone()),
            readonly,
            ..Default::default()
        }))))
    };
    app.app.scope().patch(id, field_state(false)).unwrap();
    tick(&mut app);
    app.handle_ui_event(UiEvent::Ime(BlitzImeEvent::Preedit(
        "中".into(),
        Some((3, 3)),
    )));
    tick(&mut app);
    app.app.scope().patch(id, field_state(true)).unwrap();
    tick(&mut app);
    assert!(app.renderer.0.borrow().dom.ranges[&id].editing.composing);
    app.handle_ui_event(UiEvent::Ime(BlitzImeEvent::Commit("国".into())));
    tick(&mut app);
    assert!(!app.renderer.0.borrow().dom.ranges[&id].editing.composing);
    app.renderer.inspect(|doc| {
        let editor = &doc
            .get_node(physical)
            .unwrap()
            .element_data()
            .unwrap()
            .text_input_data()
            .unwrap()
            .editor;
        assert!(!editor.is_composing());
        assert_eq!(editor.raw_text(), "draft");
    });
    app.app
        .scope()
        .patch(
            group.get().unwrap(),
            glory_render::Patch::Semantics(glory_render::Change::Set(Some(Box::new(Semantics {
                field_part: Some(FieldPart::Fieldset),
                disabled: true,
                ..Default::default()
            })))),
        )
        .unwrap();
    tick(&mut app);
    assert!(
        app.app
            .scope()
            .action(glory_render::Action::Focus(id))
            .is_err()
    );
    assert_ne!(app.renderer.0.borrow().focused, Some(id));
    assert_eq!(
        app.renderer.engine_id(id, glory_html::Part::Main).unwrap(),
        physical
    );
}

#[test]
fn native_composite_navigation_is_axis_specific_and_requires_a_registered_trigger() {
    use glory_render::{Navigation, Orientation, Semantics};
    let mut app = WindowApp::new();
    let trigger = app.app.scope().cage(None);
    let editor = app.app.scope().cage(None);
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = events.clone();
    app.mount(
        view()
            .semantics(Some(Box::new(Semantics {
                navigation: Some(Orientation::Horizontal),
                ..Default::default()
            })))
            .on(EventKind::Navigate, move |event| {
                seen.borrow_mut().push(event)
            })
            .fill((
                glory_ui::button()
                    .label("Tab")
                    .node_ref(trigger)
                    .semantics(Some(Box::new(Semantics {
                        navigation_item: true,
                        ..Default::default()
                    }))),
                input().node_ref(editor).value("text"),
            )),
    )
    .unwrap();
    tick(&mut app);
    let origin = trigger.get().unwrap();
    native_focus(&mut app, origin);
    key(&mut app, "ArrowRight", Modifiers::empty(), false, true);
    assert!(
        events.borrow().is_empty(),
        "native callbacks must only queue"
    );
    tick(&mut app);
    assert_eq!(
        *events.borrow(),
        [Event::Navigate {
            from: origin,
            command: Navigation::Next
        }]
    );
    events.borrow_mut().clear();
    for (name, modifiers) in [
        ("ArrowDown", Modifiers::empty()),
        ("Home", Modifiers::SHIFT),
    ] {
        key(&mut app, name, modifiers, false, false);
        tick(&mut app);
    }
    native_focus(&mut app, editor.get().unwrap());
    key(&mut app, "Home", Modifiers::empty(), false, false);
    tick(&mut app);
    assert!(
        events.borrow().is_empty(),
        "collection must preserve descendant editing keys"
    );
}

#[test]
fn native_horizontal_rtl_driver_reverses_arrows_and_preserves_home_end() {
    use glory_render::{Direction, Navigation, Orientation, Semantics};
    let mut app = WindowApp::new();
    let trigger = app.app.scope().cage(None);
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = events.clone();
    app.mount(
        view()
            .semantics(Some(Box::new(Semantics {
                navigation: Some(Orientation::Horizontal),
                navigation_direction: Direction::Rtl,
                ..Default::default()
            })))
            .on(EventKind::Navigate, move |event| {
                seen.borrow_mut().push(event)
            })
            .fill(
                glory_ui::button()
                    .label("RTL member")
                    .node_ref(trigger)
                    .semantics(Some(Box::new(Semantics {
                        navigation_item: true,
                        ..Default::default()
                    }))),
            ),
    )
    .unwrap();
    tick(&mut app);
    let origin = trigger.get().unwrap();
    native_focus(&mut app, origin);
    for key_name in ["ArrowLeft", "ArrowRight", "Home", "End"] {
        key(&mut app, key_name, Modifiers::empty(), false, false);
        tick(&mut app);
    }
    assert_eq!(
        *events.borrow(),
        [
            Navigation::Next,
            Navigation::Previous,
            Navigation::First,
            Navigation::Last
        ]
        .map(|command| Event::Navigate {
            from: origin,
            command
        })
    );
}

#[test]
fn native_keyboard_queues_real_commands_and_preserves_composition_and_shift_enter() {
    let mut app = WindowApp::new();
    let input_ref = app.app.scope().cage(None);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let observed = seen.clone();
    app.mount(
        view().fill(
            input()
                .multiline(true)
                .node_ref(input_ref)
                .on(EventKind::Keyboard, move |event| {
                    observed.borrow_mut().push(event)
                }),
        ),
    )
    .unwrap();
    tick(&mut app);
    let id = input_ref.get().unwrap();
    native_focus(&mut app, id);
    key(&mut app, "Enter", Modifiers::empty(), false, false);
    assert!(seen.borrow().is_empty(), "native listeners only queue");
    tick(&mut app);
    assert!(matches!(
        seen.borrow().last(),
        Some(Event::Keyboard {
            command: KeyboardCommand::Submit,
            ..
        })
    ));
    seen.borrow_mut().clear();
    for (modifiers, composing, repeat) in [
        (Modifiers::SHIFT, false, false),
        (Modifiers::ALT, false, false),
        (Modifiers::empty(), true, false),
        (Modifiers::empty(), false, true),
    ] {
        key(&mut app, "Enter", modifiers, composing, repeat);
        tick(&mut app);
        assert!(seen.borrow().is_empty());
    }
    key(&mut app, "Enter", Modifiers::CONTROL, false, false);
    tick(&mut app);
    assert!(matches!(
        seen.borrow().last(),
        Some(Event::Keyboard {
            command: KeyboardCommand::Submit,
            ..
        })
    ));
}

#[test]
fn native_drag_uses_real_pointer_hit_testing_and_delivers_drop_to_sibling() {
    let mut app = WindowApp::new();
    app.renderer.set_viewport(400, 300, 1.).unwrap();
    let a = app.app.scope().cage(None);
    let b = app.app.scope().cage(None);
    let observed = Rc::new(RefCell::new(Vec::new()));
    let left = observed.clone();
    let right = observed.clone();
    app.mount(
        view().fill((
            view()
                .node_ref(a)
                .style(
                    glory_render::style::Style::new()
                        .width(glory_render::style::Length::Px(180.))
                        .height(glory_render::style::Length::Px(80.)),
                )
                .on(EventKind::Drag, move |event| {
                    left.borrow_mut().push(("a", event))
                }),
            view()
                .node_ref(b)
                .style(
                    glory_render::style::Style::new()
                        .width(glory_render::style::Length::Px(180.))
                        .height(glory_render::style::Length::Px(80.)),
                )
                .on(EventKind::Drag, move |event| {
                    right.borrow_mut().push(("b", event))
                }),
        )),
    )
    .unwrap();
    tick(&mut app);
    let source = a.get().unwrap();
    let target = b.get().unwrap();
    let first = bounds_now(&app, source);
    let second = bounds_now(&app, target);
    let sx = (first.x + 20.) as f32;
    let sy = (first.y + 20.) as f32;
    let tx = (second.x + 40.) as f32;
    let ty = (second.y + 30.) as f32;
    app.handle_ui_event(UiEvent::PointerDown(pointer(sx, sy, true)));
    app.handle_ui_event(UiEvent::PointerMove(pointer(tx, ty, true)));
    app.handle_ui_event(UiEvent::PointerUp(pointer(tx, ty, false)));
    assert!(observed.borrow().is_empty());
    tick(&mut app);
    let observed = observed.borrow();
    assert!(
        observed.iter().any(|(owner, event)| *owner == "a"
            && matches!(event,Event::Drag{phase:DragPhase::Start,from,..} if *from==source)),
        "{observed:?}"
    );
    assert!(
        observed.iter().any(|(owner, event)| *owner == "b"
            && matches!(event,Event::Drag{phase:DragPhase::Drop,from,..} if *from==source)),
        "{observed:?}"
    );
}

#[test]
fn native_color_scheme_is_queued_coalesced_and_initializes_late_view_interests() {
    let mut app = WindowApp::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let first = seen.clone();
    app.mount(view().on(EventKind::ColorSchemeChanged, move |event| {
        first.borrow_mut().push(event)
    }))
    .unwrap();
    queue_color_scheme(&mut app.renderer.0.borrow_mut(), true);
    queue_color_scheme(&mut app.renderer.0.borrow_mut(), true);
    assert!(seen.borrow().is_empty());
    tick(&mut app);
    assert_eq!(&*seen.borrow(), &[Event::ColorSchemeChanged { dark: true }]);
    let second = seen.clone();
    let late = view()
        .on(EventKind::ColorSchemeChanged, move |event| {
            second.borrow_mut().push(event)
        })
        .mount(app.app.scope())
        .unwrap();
    tick(&mut app);
    assert_eq!(seen.borrow().len(), 2);
    drop(late);
    queue_color_scheme(&mut app.renderer.0.borrow_mut(), false);
    tick(&mut app);
    assert_eq!(seen.borrow().len(), 3);
    assert_eq!(
        seen.borrow().last(),
        Some(&Event::ColorSchemeChanged { dark: false })
    );
}

#[test]
fn native_toggle_button_space_enter_queue_activation_and_keep_pressed_engine_identity() {
    use glory_render::Semantics;
    let mut app = WindowApp::new();
    let reference = app.app.scope().cage(None);
    let pressed = app.app.scope().cage(false);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let trace = seen.clone();
    let metadata = app.app.scope().bond(move || {
        Some(Box::new(Semantics {
            pressed: Some(pressed.get()),
            ..Default::default()
        }))
    });
    app.mount(
        glory_ui::button()
            .label("Bold")
            .node_ref(reference)
            .semantics(metadata)
            .on(EventKind::Activate, move |_| {
                pressed.revise(|value| *value = !*value);
                trace.borrow_mut().push(pressed.get());
            }),
    )
    .unwrap();
    tick(&mut app);
    let button = reference.get().unwrap();
    native_focus(&mut app, button);
    let physical = app
        .renderer
        .engine_id(button, glory_html::Part::Main)
        .unwrap();
    key(&mut app, "Enter", Modifiers::empty(), false, false);
    assert!(seen.borrow().is_empty());
    tick(&mut app);
    assert_eq!(&*seen.borrow(), &[true]);
    key(&mut app, " ", Modifiers::empty(), false, false);
    tick(&mut app);
    assert_eq!(seen.borrow().len(), 1, "Space arms until release");
    app.handle_ui_event(UiEvent::KeyUp(BlitzKeyEvent {
        key: Key::Character(" ".into()),
        code: keyboard_types::Code::Space,
        modifiers: Modifiers::empty(),
        location: keyboard_types::Location::Standard,
        is_auto_repeating: false,
        is_composing: false,
        state: blitz_traits::events::KeyState::Released,
        text: None,
    }));
    assert_eq!(seen.borrow().len(), 1, "release only queues");
    tick(&mut app);
    assert_eq!(&*seen.borrow(), &[true, false]);
    assert_eq!(
        app.renderer
            .engine_id(button, glory_html::Part::Main)
            .unwrap(),
        physical
    );
    assert_eq!(
        app.app
            .tree()
            .node(button)
            .unwrap()
            .descriptor
            .view()
            .semantics
            .as_ref()
            .unwrap()
            .pressed,
        Some(false)
    );
}

#[test]
fn native_splitter_captures_release_to_original_handle_and_queues_own_range_keys() {
    use glory_render::{
        FocusMode, Orientation, RangeCommand, RangeValue, Role, SemanticId, Semantics,
    };
    let mut app = WindowApp::new();
    app.renderer.set_viewport(400, 300, 1.0).unwrap();
    let reference = app.app.scope().cage(None);
    let observed = Rc::new(RefCell::new(Vec::new()));
    let output = observed.clone();
    let sibling = observed.clone();
    let range = observed.clone();
    app.mount(
        view().fill((
            view()
                .role(Role::Separator)
                .focus(FocusMode::TabStop)
                .node_ref(reference)
                .semantics(Some(Box::new(Semantics {
                    splitter: Some(RangeValue {
                        min: 0.0,
                        max: 100.0,
                        value: 50.0,
                    }),
                    separator_axis: Some(Orientation::Vertical),
                    controls: Some(SemanticId::new("pane").unwrap()),
                    ..Default::default()
                })))
                .style(
                    glory_render::style::Style::new()
                        .width(glory_render::style::Length::Px(12.0))
                        .height(glory_render::style::Length::Px(80.0)),
                )
                .on(EventKind::Drag, move |event| {
                    output.borrow_mut().push(("handle", event))
                })
                .on(EventKind::Range, move |event| {
                    range.borrow_mut().push(("range", event))
                }),
            view()
                .style(
                    glory_render::style::Style::new()
                        .width(glory_render::style::Length::Px(180.0))
                        .height(glory_render::style::Length::Px(80.0)),
                )
                .on(EventKind::Drag, move |event| {
                    sibling.borrow_mut().push(("sibling", event))
                }),
        )),
    )
    .unwrap();
    tick(&mut app);
    let handle = reference.get().unwrap();
    native_focus(&mut app, handle);
    key(&mut app, "ArrowRight", Modifiers::empty(), false, false);
    key(&mut app, "End", Modifiers::empty(), false, false);
    assert!(observed.borrow().is_empty());
    tick(&mut app);
    assert!(observed.borrow().iter().any(|(_, event)| matches!(
        event,
        Event::Range {
            command: RangeCommand::Increase,
            ..
        }
    )));
    observed.borrow_mut().clear();
    let rect = bounds_now(&app, handle);
    let x = (rect.x + 5.0) as f32;
    let y = (rect.y + 20.0) as f32;
    app.handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    app.handle_ui_event(UiEvent::PointerMove(pointer(120.0, 130.0, true)));
    app.handle_ui_event(UiEvent::PointerUp(pointer(120.0, 130.0, false)));
    assert!(observed.borrow().is_empty());
    tick(&mut app);
    assert!(
        observed
            .borrow()
            .iter()
            .any(|(owner, event)| *owner == "handle"
                && matches!(event,Event::Drag{phase:DragPhase::Drop,from,..}if *from==handle)),
        "{:?}",
        observed.borrow()
    );
    assert!(
        !observed
            .borrow()
            .iter()
            .any(|(owner, _)| *owner == "sibling")
    );
    observed.borrow_mut().clear();
    let finger = |x, y, down| {
        let mut event = pointer(x, y, down);
        event.id = blitz_traits::events::BlitzPointerId::Finger(7);
        event
    };
    app.handle_ui_event(UiEvent::PointerDown(finger(x, y, true)));
    app.handle_ui_event(UiEvent::PointerMove(finger(120.0, 130.0, true)));
    app.handle_ui_event(UiEvent::PointerUp(finger(120.0, 130.0, false)));
    assert!(observed.borrow().is_empty());
    tick(&mut app);
    assert!(observed.borrow().iter().any(|(owner, event)| *owner == "handle"
        && matches!(event, Event::Drag { phase: DragPhase::Drop, from, .. } if *from == handle)));
    observed.borrow_mut().clear();
    app.handle_ui_event(UiEvent::PointerDown(finger(x, y, true)));
    app.handle_ui_event(UiEvent::PointerMove(finger(120.0, 130.0, true)));
    app.handle_ui_event(UiEvent::PointerCancel(finger(120.0, 130.0, false)));
    tick(&mut app);
    assert!(observed.borrow().iter().any(|(_, event)| matches!(
        event,
        Event::Drag {
            phase: DragPhase::Cancel,
            ..
        }
    )));
    assert!(!observed.borrow().iter().any(|(_, event)| matches!(
        event,
        Event::Drag {
            phase: DragPhase::Drop,
            ..
        }
    )));
}

#[test]
fn native_navigation_panel_keys_queue_only_for_opted_in_controls_and_layer_open_is_rejected() {
    use glory_render::{
        Change, Layer, LayerFocus, LayerPlacement, NavigationPanel, NavigationPanelCommand,
        Orientation, Patch, Role, SemanticId, Semantics,
    };
    let mut app = WindowApp::new();
    let scope = app.app.scope();
    let trigger = scope.cage(None);
    let panel = scope.cage(None);
    let link = scope.cage(None);
    let ordinary = scope.cage(None);
    let observed = Rc::new(RefCell::new(Vec::new()));
    let header = observed.clone();
    let content = observed.clone();
    app.mount(
        view().fill((
            glory_ui::button()
                .label("Products")
                .node_ref(trigger)
                .semantics(Some(Box::new(Semantics {
                    id: Some(SemanticId::new("nav-trigger").unwrap()),
                    controls: Some(SemanticId::new("nav-panel").unwrap()),
                    expanded: Some(false),
                    navigation_item: true,
                    navigation_panel: Some(NavigationPanel::Trigger),
                    ..Default::default()
                })))
                .on(EventKind::NavigationPanel, move |event| {
                    header.borrow_mut().push(event)
                }),
            view()
                .role(Role::Group)
                .name("Product links")
                .node_ref(panel)
                .semantics(Some(Box::new(Semantics {
                    id: Some(SemanticId::new("nav-panel").unwrap()),
                    navigation: Some(Orientation::Vertical),
                    navigation_panel: Some(NavigationPanel::Content),
                    ..Default::default()
                })))
                .on(EventKind::NavigationPanel, move |event| {
                    content.borrow_mut().push(event)
                })
                .fill((
                    glory_ui::link()
                        .target("/docs")
                        .label("Docs")
                        .node_ref(link)
                        .semantics(Some(Box::new(Semantics {
                            navigation_item: true,
                            ..Default::default()
                        }))),
                    glory_ui::button()
                        .label("Ordinary control")
                        .node_ref(ordinary),
                )),
        )),
    )
    .unwrap();
    tick(&mut app);
    let trigger = trigger.get().unwrap();
    native_focus(&mut app, trigger);
    key(&mut app, "ArrowDown", Modifiers::empty(), false, false);
    key(&mut app, "ArrowUp", Modifiers::empty(), false, false);
    assert!(observed.borrow().is_empty());
    tick(&mut app);
    assert!(matches!(
        &observed.borrow()[..],
        [
            Event::NavigationPanel {
                command: NavigationPanelCommand::First,
                ..
            },
            Event::NavigationPanel {
                command: NavigationPanelCommand::Last,
                ..
            }
        ]
    ));
    observed.borrow_mut().clear();
    native_focus(&mut app, link.get().unwrap());
    key(&mut app, "ArrowLeft", Modifiers::empty(), false, false);
    assert!(observed.borrow().is_empty());
    tick(&mut app);
    assert!(matches!(
        &observed.borrow()[..],
        [Event::NavigationPanel {
            command: NavigationPanelCommand::Close,
            ..
        }]
    ));
    observed.borrow_mut().clear();
    native_focus(&mut app, ordinary.get().unwrap());
    key(&mut app, "ArrowLeft", Modifiers::empty(), false, false);
    tick(&mut app);
    assert!(observed.borrow().is_empty());
    let panel = panel.get().unwrap();
    let mut metadata = app
        .app
        .tree()
        .node(panel)
        .unwrap()
        .descriptor
        .view()
        .semantics
        .as_deref()
        .unwrap()
        .clone();
    metadata.layer = Some(Layer {
        modal: false,
        focus: LayerFocus::Preserve,
        placement: LayerPlacement::Anchor(glory_render::AnchorPlacement::new(
            glory_render::AnchorSide::Bottom,
            glory_render::AnchorAlign::Start,
        )),
        anchor: Some(SemanticId::new("nav-trigger").unwrap()),
        ..Default::default()
    });
    let before = (app.app.stats(), app.renderer.stats());
    assert!(
        app.app
            .scope()
            .patch(
                panel,
                Patch::Semantics(Change::Set(Some(Box::new(metadata))))
            )
            .is_err()
    );
    assert_eq!((app.app.stats(), app.renderer.stats()), before);
}

#[test]
fn native_slider_queues_axis_rtl_page_keys_and_captures_mouse_and_touch_release_cancel() {
    use glory_render::{
        Direction, FocusMode, Orientation, RangeCommand, RangeValue, Role, Semantics, SliderValue,
    };
    let mut app = WindowApp::new();
    app.renderer.set_viewport(500, 400, 1.).unwrap();
    let reference = app.app.scope().cage(None);
    let events = Rc::new(RefCell::new(Vec::new()));
    let ranges = events.clone();
    let drags = events.clone();
    app.mount(
        view()
            .role(Role::Slider)
            .focus(FocusMode::TabStop)
            .name("RTL volume")
            .node_ref(reference)
            .semantics(Some(Box::new(Semantics {
                slider: Some(SliderValue {
                    range: RangeValue {
                        min: 0.,
                        max: 100.,
                        value: 50.,
                    },
                    axis: Orientation::Horizontal,
                    direction: Direction::Rtl,
                    step: 1.,
                    page_step: 10.,
                    value_text: None,
                }),
                ..Default::default()
            })))
            .style(
                glory_render::style::Style::new()
                    .width(glory_render::style::Length::Px(200.))
                    .height(glory_render::style::Length::Px(30.)),
            )
            .on(EventKind::Range, move |event| {
                ranges.borrow_mut().push(event)
            })
            .on(EventKind::Drag, move |event| drags.borrow_mut().push(event)),
    )
    .unwrap();
    tick(&mut app);
    let id = reference.get().unwrap();
    native_focus(&mut app, id);
    key(&mut app, "ArrowRight", Modifiers::empty(), false, false);
    key(&mut app, "PageUp", Modifiers::empty(), false, false);
    key(&mut app, "ArrowUp", Modifiers::empty(), false, false);
    assert!(events.borrow().is_empty());
    tick(&mut app);
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter_map(|event| if let Event::Range { command, .. } = event {
                Some(*command)
            } else {
                None
            })
            .collect::<Vec<_>>(),
        vec![RangeCommand::Decrease, RangeCommand::PageIncrease]
    );
    events.borrow_mut().clear();
    let rect = bounds_now(&app, id);
    let (x, y) = ((rect.x + 40.) as f32, (rect.y + 15.) as f32);
    for touch in [false, true] {
        let sample = |x, y, down| {
            let mut event = pointer(x, y, down);
            if touch {
                event.id = blitz_traits::events::BlitzPointerId::Finger(9)
            }
            event
        };
        app.handle_ui_event(UiEvent::PointerDown(sample(x, y, true)));
        app.handle_ui_event(UiEvent::PointerMove(sample(350., 180., true)));
        app.handle_ui_event(UiEvent::PointerUp(sample(350., 180., false)));
        assert!(events.borrow().is_empty());
        tick(&mut app);
        assert!(
            events
                .borrow()
                .iter()
                .any(|e| matches!(e,Event::Drag{from,phase:DragPhase::Start,..}if *from==id))
        );
        assert!(
            events
                .borrow()
                .iter()
                .any(|e| matches!(e,Event::Drag{from,phase:DragPhase::Drop,..}if *from==id))
        );
        events.borrow_mut().clear();
    }
    let sample = |x, y, down| {
        let mut event = pointer(x, y, down);
        event.id = blitz_traits::events::BlitzPointerId::Finger(10);
        event
    };
    app.handle_ui_event(UiEvent::PointerDown(sample(x, y, true)));
    app.handle_ui_event(UiEvent::PointerMove(sample(350., 180., true)));
    app.handle_ui_event(UiEvent::PointerCancel(sample(350., 180., false)));
    tick(&mut app);
    assert!(events.borrow().iter().any(|e| matches!(
        e,
        Event::Drag {
            phase: DragPhase::Cancel,
            ..
        }
    )));
    assert!(!events.borrow().iter().any(|e| matches!(
        e,
        Event::Drag {
            phase: DragPhase::Drop,
            ..
        }
    )));
}

#[test]
fn native_view_press_capture_preserves_editor_focus_and_queues_mouse_touch_and_cancel() {
    use glory_render::style::{Length, Style};
    use glory_render::{DragCapture, FocusMode, Semantics};
    for policy in [DragCapture::Press, DragCapture::PressFocus] {
        let mut app = WindowApp::new();
        app.renderer.set_viewport(500, 400, 1.).unwrap();
        let editor = app.app.scope().cage(None);
        let target = app.app.scope().cage(None);
        let events = Rc::new(RefCell::new(Vec::new()));
        let seen = events.clone();
        app.mount(
            view().fill((
                input().value("preserve editor").node_ref(editor),
                view()
                    .node_ref(target)
                    .focus(FocusMode::Programmatic)
                    .semantics(Some(Box::new(Semantics {
                        drag_capture: Some(policy),
                        ..Default::default()
                    })))
                    .style(
                        Style::new()
                            .width(Length::Px(200.))
                            .height(Length::Px(100.)),
                    )
                    .on(EventKind::Drag, move |event| seen.borrow_mut().push(event)),
            )),
        )
        .unwrap();
        tick(&mut app);
        let editor = editor.get().unwrap();
        let target = target.get().unwrap();
        native_focus(&mut app, editor);
        tick(&mut app);
        let rect = bounds_now(&app, target);
        let x = (rect.x + 30.) as f32;
        let y = (rect.y + 20.) as f32;
        for touch in [false, true] {
            events.borrow_mut().clear();
            let sample = |x, y, down| {
                let mut event = pointer(x, y, down);
                if touch {
                    event.id = blitz_traits::events::BlitzPointerId::Finger(12);
                }
                event
            };
            app.handle_ui_event(UiEvent::PointerDown(sample(x, y, true)));
            assert!(events.borrow().is_empty());
            tick(&mut app);
            assert!(
                matches!(&events.borrow()[0], Event::Drag { from, phase: DragPhase::Start, .. } if *from == target)
            );
            let state = app.renderer.0.borrow();
            let focused = state
                .dom
                .owner(state.dom.doc.get_focussed_node_id().unwrap())
                .unwrap();
            assert_eq!(
                focused,
                if policy == DragCapture::Press {
                    editor
                } else {
                    target
                }
            );
            drop(state);
            events.borrow_mut().clear();
            app.handle_ui_event(UiEvent::PointerMove(sample(380., 300., true)));
            app.handle_ui_event(UiEvent::PointerUp(sample(380., 300., false)));
            assert!(events.borrow().is_empty());
            tick(&mut app);
            assert!(events.borrow().iter().any(|event| matches!(event, Event::Drag { from, phase: DragPhase::Drop, .. } if *from == target)));
            events.borrow_mut().clear();
            app.handle_ui_event(UiEvent::PointerDown(sample(x, y, true)));
            app.handle_ui_event(UiEvent::PointerCancel(sample(380., 300., false)));
            tick(&mut app);
            assert!(events.borrow().iter().any(|event| matches!(
                event,
                Event::Drag {
                    phase: DragPhase::Cancel,
                    ..
                }
            )));
            assert!(!events.borrow().iter().any(|event| matches!(
                event,
                Event::Drag {
                    phase: DragPhase::Drop,
                    ..
                }
            )));
        }
    }
}

#[test]
fn native_spinbutton_queues_own_focused_keys_without_changing_editor_or_ordinary_inputs() {
    use glory_render::{RangeCommand, Semantics, SpinValue};
    let mut app = WindowApp::new();
    app.renderer.set_viewport(500, 300, 1.).unwrap();
    let reference = app.app.scope().cage(None);
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    app.mount(
        input()
            .value("0.00")
            .name("Quantity")
            .node_ref(reference)
            .semantics(Some(Box::new(Semantics {
                spinbutton: Some(SpinValue {
                    min: Some(0.),
                    max: Some(10.),
                    value: Some(0.),
                    can_decrease: false,
                    ..Default::default()
                }),
                ..Default::default()
            })))
            .on(EventKind::Spin, move |e| output.borrow_mut().push(e)),
    )
    .unwrap();
    tick(&mut app);
    let id = reference.get().unwrap();
    native_focus(&mut app, id);
    key(&mut app, "ArrowUp", Modifiers::empty(), false, false);
    key(&mut app, "PageDown", Modifiers::empty(), false, false);
    key(&mut app, "Home", Modifiers::empty(), false, false);
    key(&mut app, "End", Modifiers::empty(), false, false);
    key(&mut app, "ArrowUp", Modifiers::SHIFT, false, false);
    key(&mut app, "ArrowUp", Modifiers::empty(), true, false);
    assert!(events.borrow().is_empty());
    tick(&mut app);
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter_map(|e| if let Event::Spin { command, .. } = e {
                Some(*command)
            } else {
                None
            })
            .collect::<Vec<_>>(),
        vec![
            RangeCommand::Increase,
            RangeCommand::PageDecrease,
            RangeCommand::Minimum,
            RangeCommand::Maximum
        ]
    );
    assert_eq!(app.renderer.0.borrow().dom.text_value(id).unwrap(), "0.00");
    let ordinary = app.app.scope().cage(None);
    app.mount(input().value("plain").node_ref(ordinary))
        .unwrap();
    tick(&mut app);
    native_focus(&mut app, ordinary.get().unwrap());
    events.borrow_mut().clear();
    key(&mut app, "ArrowUp", Modifiers::empty(), false, false);
    tick(&mut app);
    assert!(events.borrow().is_empty());
}

#[test]
fn native_single_line_enter_queues_commit_before_form_submit_and_keeps_textarea_and_ime_policy() {
    use glory_render::{FieldPart, Semantics};
    let mut app = WindowApp::new();
    let reference = app.app.scope().cage(None);
    let source = app.app.scope().cage("20".to_owned());
    let events = Rc::new(RefCell::new(Vec::new()));
    let input_events = events.clone();
    let form_events = events.clone();
    let output = source;
    app.mount(
        view()
            .field_part(FieldPart::Form)
            .on(EventKind::Submit, move |_| {
                form_events
                    .borrow_mut()
                    .push(format!("submit:{}", source.get()))
            })
            .fill(
                input()
                    .value(source)
                    .node_ref(reference)
                    .on(EventKind::Commit, move |event| {
                        if let Event::Commit { value, .. } = event {
                            input_events.borrow_mut().push(format!("commit:{value}"));
                            output.revise(|v| *v = "10".into());
                        }
                    }),
            ),
    )
    .unwrap();
    tick(&mut app);
    let id = reference.get().unwrap();
    native_focus(&mut app, id);
    key(&mut app, "Enter", Modifiers::empty(), false, false);
    assert!(events.borrow().is_empty());
    tick(&mut app);
    assert_eq!(events.borrow().as_slice(), ["commit:20", "submit:10"]);
    events.borrow_mut().clear();
    key(&mut app, "Enter", Modifiers::empty(), true, false);
    tick(&mut app);
    assert!(events.borrow().is_empty());
    let textarea = app.app.scope().cage(None);
    let out = events.clone();
    app.mount(
        view()
            .semantics(Some(Box::new(Semantics {
                field_part: Some(FieldPart::Form),
                ..Default::default()
            })))
            .fill(
                input()
                    .multiline(true)
                    .node_ref(textarea)
                    .on(EventKind::Commit, move |_| {
                        out.borrow_mut().push("textarea commit".into())
                    }),
            ),
    )
    .unwrap();
    tick(&mut app);
    native_focus(&mut app, textarea.get().unwrap());
    key(&mut app, "Enter", Modifiers::empty(), false, false);
    tick(&mut app);
    assert!(events.borrow().is_empty());
}

#[test]
fn native_tree_commands_are_queued_cross_groups_and_leave_nested_editors_and_chords_native() {
    use super::focus_tests::focus;
    use glory_render::{
        Direction, FocusMode, Role, Semantics, TreeCommand, TreeItem, TreeNavigation,
    };
    let mut host = WindowApp::new();
    let item = host.app.scope().cage(None);
    let editor = host.app.scope().cage(None);
    let commands = Rc::new(RefCell::new(Vec::new()));
    let out = commands.clone();
    host.mount(
        view()
            .role(Role::Tree)
            .name("Files")
            .semantics(Some(Box::new(Semantics {
                tree_navigation: Some(TreeNavigation {
                    direction: Direction::Rtl,
                    ..Default::default()
                }),
                ..Default::default()
            })))
            .on(EventKind::TreeNavigate, move |event| {
                if let Event::TreeNavigate { command, .. } = event {
                    out.borrow_mut().push(command)
                }
            })
            .fill(
                glory_ui::group().fill(
                    view()
                        .role(Role::TreeItem)
                        .name("Source")
                        .focus(FocusMode::TabStop)
                        .node_ref(item)
                        .semantics(Some(Box::new(Semantics {
                            tree_item: Some(TreeItem {
                                level: 1,
                                position: 0,
                                count: 1,
                            }),
                            expanded: Some(false),
                            selected: Some(false),
                            ..Default::default()
                        })))
                        .fill(input().node_ref(editor).value("draft")),
                ),
            ),
    )
    .unwrap();
    focus(&mut host, item.get().unwrap());
    key(&mut host, "ArrowLeft", Modifiers::empty(), false, false);
    assert!(commands.borrow().is_empty());
    tick(&mut host);
    assert_eq!(commands.borrow().as_slice(), [TreeCommand::Open]);
    key(&mut host, "l", Modifiers::empty(), false, false);
    tick(&mut host);
    assert!(matches!(commands.borrow().last(),Some(TreeCommand::Search{text,..}) if text=="l"));
    for modifiers in [
        Modifiers::CONTROL,
        Modifiers::ALT,
        Modifiers::META,
        Modifiers::SHIFT,
    ] {
        key(&mut host, "ArrowDown", modifiers, false, false);
        tick(&mut host);
    }
    key(&mut host, "ArrowDown", Modifiers::empty(), true, false);
    tick(&mut host);
    assert_eq!(commands.borrow().len(), 2);
    focus(&mut host, editor.get().unwrap());
    key(&mut host, "ArrowLeft", Modifiers::empty(), false, false);
    tick(&mut host);
    assert_eq!(commands.borrow().len(), 2);
    host.app.clear().unwrap();
    assert!(item.get().is_none());
}
