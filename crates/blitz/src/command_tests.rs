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
fn native_dismiss_bubbles_from_editor_without_stealing_submit_or_ime() {
    let mut app = WindowApp::new();
    let input_ref = app.app.scope().cage(None);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let observed = seen.clone();
    app.mount(
        view()
            .on(EventKind::Keyboard, move |event| {
                observed.borrow_mut().push(event);
            })
            .fill(input().multiline(true).node_ref(input_ref)),
    )
    .unwrap();
    tick(&mut app);
    let id = input_ref.get().unwrap();
    native_focus(&mut app, id);
    key(&mut app, "Enter", Modifiers::empty(), false, false);
    tick(&mut app);
    assert!(
        seen.borrow().is_empty(),
        "ancestor interest must not steal editor Enter"
    );
    assert_eq!(app.renderer.0.borrow().dom.text_value(id).unwrap(), "\n");
    for (modifiers, composing, repeat) in [
        (Modifiers::empty(), true, false),
        (Modifiers::ALT, false, false),
        (Modifiers::empty(), false, true),
    ] {
        key(&mut app, "Escape", modifiers, composing, repeat);
        tick(&mut app);
        assert!(seen.borrow().is_empty(), "filtered Escape must not dismiss");
    }
    key(&mut app, "Escape", Modifiers::empty(), false, false);
    assert!(seen.borrow().is_empty(), "host callback must only enqueue");
    tick(&mut app);
    assert!(
        matches!(seen.borrow().as_slice(), [Event::Keyboard {from, command: KeyboardCommand::Dismiss}] if *from == id)
    );
}
