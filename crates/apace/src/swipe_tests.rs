use super::*;
use glory_render::{DragPhase, Orientation, Semantics, SwipeCapture};

#[test]
fn actual_native_mouse_axis_intent_revision_cancel_and_editor_exclusion() {
    let mut app = WindowApp::new();
    let configuration = app.app.scope().cage(Some(SwipeCapture {
        axis: Orientation::Horizontal,
        revision: 1,
    }));
    let log = Rc::new(RefCell::new(Vec::new()));
    let output = log.clone();
    app.mount(
        view()
            .semantics(configuration.map(|capture| {
                Some(Box::new(Semantics {
                    swipe_capture: *capture,
                    ..Default::default()
                }))
            }))
            .style(
                glory_render::style::Style::new()
                    .width(glory_render::style::Length::Px(400.))
                    .height(glory_render::style::Length::Px(300.)),
            )
            .on(EventKind::Swipe, move |event| {
                if let Event::Swipe {
                    phase, revision, ..
                } = event
                {
                    output.borrow_mut().push((phase, revision));
                }
            })
            .fill(input().value("Editor")),
    )
    .unwrap();
    tick(&mut app);
    let region = app.app.tree().roots()[0];
    let rect = bounds_now(&app, region);
    let x = (rect.x + 80.) as f32;
    let y = (rect.y + 120.) as f32;
    app.handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    app.handle_ui_event(UiEvent::PointerMove(pointer(x + 3., y + 25., true)));
    app.handle_ui_event(UiEvent::PointerUp(pointer(x + 3., y + 25., false)));
    tick(&mut app);
    assert!(log.borrow().is_empty(), "orthogonal pan is not a swipe");
    app.handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    app.handle_ui_event(UiEvent::PointerMove(pointer(x + 30., y + 2., true)));
    app.handle_ui_event(UiEvent::PointerUp(pointer(x + 50., y + 2., false)));
    tick(&mut app);
    assert_eq!(
        *log.borrow(),
        vec![
            (DragPhase::Start, 1),
            (DragPhase::Move, 1),
            (DragPhase::Drop, 1)
        ]
    );
    log.borrow_mut().clear();
    app.handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    app.handle_ui_event(UiEvent::PointerMove(pointer(x + 30., y, true)));
    tick(&mut app);
    configuration.revise(|capture| *capture = None);
    tick(&mut app);
    assert!(app.renderer.0.borrow().swipe.is_none());
    // Captured old revision is silently discarded after its declaration clears.
    assert_eq!(log.borrow().len(), 2);
    configuration.revise(|capture| {
        *capture = Some(SwipeCapture {
            axis: Orientation::Horizontal,
            revision: 2,
        })
    });
    tick(&mut app);
    log.borrow_mut().clear();
    let editor = app.app.tree().node(region).unwrap().children[0];
    let rect = bounds_now(&app, editor);
    let x = (rect.x + 8.) as f32;
    let y = (rect.y + 8.) as f32;
    app.handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    app.handle_ui_event(UiEvent::PointerMove(pointer(x + 45., y, true)));
    app.handle_ui_event(UiEvent::PointerUp(pointer(x + 45., y, false)));
    tick(&mut app);
    assert!(log.borrow().is_empty());
}
