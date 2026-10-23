use super::tests::{bounds_now, native_focus, pointer, tick};
use super::*;
use glory_render::{DragPhase, KeyboardCommand};
use glory_ui::{input, view};

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
