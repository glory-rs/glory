use super::*;
use glory_render::style::{Length, Style};
use glory_render::{Action, EventKind, Semantics};
#[test]
fn native_inert_prevents_pointer_caret_edits_tab_and_business_callbacks_without_a_scroll_lock() {
    let mut app = WindowApp::new();
    let blocked = app.app.scope().cage(false);
    let value = app.app.scope().cage("Retained".to_string());
    let calls = app.app.scope().cage(0usize);
    app.mount(
        view().fill((
            view()
                .semantics(blocked.map(|blocked| {
                    Some(Box::new(Semantics {
                        inert: *blocked,
                        ..Default::default()
                    }))
                }))
                .style(
                    Style::new()
                        .width(Length::Px(100.))
                        .height(Length::Px(100.))
                        .overflow_y(glory_render::style::Overflow::Auto),
                )
                .fill(
                    view().style(Style::new().height(Length::Px(400.))).fill((
                        input()
                            .value(value)
                            .style(Style::new().width(Length::Px(80.)).height(Length::Px(30.)))
                            .on(EventKind::ValueChanged, move |event| {
                                if let Event::ValueChanged { value: next, .. } = event {
                                    calls.revise(|n| *n += 1);
                                    value.revise(|s| *s = next)
                                }
                            }),
                        button()
                            .label("Inside")
                            .on(EventKind::Activate, move |_| calls.revise(|n| *n += 10)),
                    )),
                ),
            button().label("Outside"),
        )),
    )
    .unwrap();
    tick(&mut app);
    let root = app.app.tree().roots()[0];
    let region = app.app.tree().node(root).unwrap().children[0];
    let content = app.app.tree().node(region).unwrap().children[0];
    let editor = app.app.tree().node(content).unwrap().children[0];
    let outside = app.app.tree().node(root).unwrap().children[1];
    native_focus(&mut app, editor);
    let selection = selection_now(&app, editor);
    blocked.revise(|v| *v = true);
    tick(&mut app);
    assert!(app.renderer.0.borrow().focused.is_none());
    let rect = bounds_now(&app, editor);
    let x = (rect.x + rect.width * 0.8) as f32;
    let y = (rect.y + rect.height / 2.) as f32;
    app.handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    app.handle_ui_event(UiEvent::PointerUp(pointer(x, y, false)));
    tick(&mut app);
    assert_eq!(selection_now(&app, editor), selection);
    assert!(app.renderer.0.borrow().focused.is_none());
    native_key(&mut app, Key::Character("x".into()));
    tick(&mut app);
    assert_eq!(calls.get(), 0);
    assert_eq!(value.get(), "Retained");
    assert_eq!(
        app.renderer.0.borrow().dom.text_value(editor).unwrap(),
        "Retained"
    );
    native_key(&mut app, Key::Named(NamedKey::Tab));
    tick(&mut app);
    assert_eq!(native_focused(&app), outside);
    app.app
        .scope()
        .action(Action::ScrollTo {
            node: region,
            x: 0.,
            y: 50.,
        })
        .unwrap();
    app.app.flush().unwrap();
    let physical = app.renderer.engine_id(region, Part::Main).unwrap();
    assert!(
        app.renderer
            .inspect(|doc| doc.get_node(physical).unwrap().scroll_offset().y)
            > 0.
    );
    blocked.revise(|v| *v = false);
    tick(&mut app);
    assert_eq!(native_focused(&app), outside);
    native_focus(&mut app, editor);
    native_key(&mut app, Key::Character("x".into()));
    tick(&mut app);
    assert_eq!(calls.get(), 1);
    assert!(value.get().contains('x'));
}
