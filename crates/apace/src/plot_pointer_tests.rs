//! Actual engine protocol ingress, not OS pointer/device acceptance.
use super::tests::{bounds_now, pointer, tick};
use super::*;
use glory_render::style::{Length, Style};
use glory_render::{PlotPointer, PlotPointerDevice as Device, PlotPointerPhase as Phase};
use glory_ui::view;
type Log = Rc<RefCell<Vec<(NodeId, PlotPointer)>>>;
fn fixture() -> (WindowApp, NodeId, Log) {
    let mut app = WindowApp::new();
    let reference = app.app.scope().cage(None);
    let log = Rc::new(RefCell::new(Vec::new()));
    let output = log.clone();
    app.mount(
        view()
            .class("plot")
            .node_ref(reference)
            .style(
                Style::new()
                    .width(Length::Px(240.25))
                    .height(Length::Px(160.5)),
            )
            .on(EventKind::PlotPointer, move |event| {
                if let Event::PlotPointer { from, pointer } = event {
                    output.borrow_mut().push((from, pointer));
                }
            }),
    )
    .unwrap();
    tick(&mut app);
    let id = reference.get().unwrap();
    (app, id, log)
}
#[test]
fn actual_ui_fractional_css_viewport_is_queued_and_not_device_scaled() {
    for scale in [1., 2.] {
        let (mut app, id, log) = fixture();
        app.renderer.set_viewport(1000, 800, scale).unwrap();
        tick(&mut app);
        let b = bounds_now(&app, id);
        let x = (b.x + 12.375) as f32;
        let y = (b.y + 9.125) as f32;
        app.handle_ui_event(UiEvent::PointerMove(pointer(x, y, false)));
        assert!(log.borrow().is_empty());
        tick(&mut app);
        let events = log.borrow();
        let (from, p) = events.last().unwrap();
        assert_eq!(*from, id);
        assert_eq!(p.phase, Phase::Move);
        assert_eq!(p.device, Device::Mouse);
        assert_eq!(p.x, f64::from(x) - b.x);
        assert_eq!(p.y, f64::from(y) - b.y);
        assert_eq!(
            p.viewport,
            Rect {
                x: 0.,
                y: 0.,
                width: b.width,
                height: b.height
            }
        );
    }
}
#[test]
fn actual_ui_capture_keeps_original_view_for_outside_move_and_up() {
    let (mut app, id, log) = fixture();
    let b = bounds_now(&app, id);
    app.handle_ui_event(UiEvent::PointerDown(pointer(
        (b.x + 10.375) as f32,
        (b.y + 8.125) as f32,
        true,
    )));
    tick(&mut app);
    app.handle_ui_event(UiEvent::PointerMove(pointer(
        (b.x + b.width + 25.25) as f32,
        (b.y + 30.5) as f32,
        true,
    )));
    app.handle_ui_event(UiEvent::PointerUp(pointer(
        (b.x + b.width + 25.25) as f32,
        (b.y + 30.5) as f32,
        false,
    )));
    tick(&mut app);
    let events = log.borrow();
    assert!(events.iter().all(|(from, _)| *from == id));
    assert_eq!(
        events
            .iter()
            .filter(|(_, p)| p.phase == Phase::Down)
            .count(),
        1
    );
    assert_eq!(
        events.iter().filter(|(_, p)| p.phase == Phase::Up).count(),
        1
    );
    assert!(
        events
            .iter()
            .any(|(_, p)| p.phase == Phase::Move && p.x > b.width)
    );
}
#[test]
fn real_ui_and_direct_dom_paths_do_not_duplicate_one_dispatch() {
    let (mut app, id, log) = fixture();
    let b = bounds_now(&app, id);
    let mut point = pointer((b.x + 17.375) as f32, (b.y + 14.125) as f32, false);
    point.id = apace_traits::events::ApacePointerId::Pen;
    app.handle_ui_event(UiEvent::PointerMove(point.clone()));
    tick(&mut app);
    assert_eq!(
        log.borrow()
            .iter()
            .filter(|(_, p)| p.phase == Phase::Move)
            .count(),
        1
    );
    log.borrow_mut().clear();
    let target = app.renderer.0.borrow().dom.main(id).unwrap();
    let collector = Events::new(app.renderer.clone());
    EventDriver::new(&mut app, collector)
        .handle_dom_event(DomEvent::new(target, DomEventData::PointerMove(point)));
    assert!(log.borrow().is_empty());
    tick(&mut app);
    let events = log.borrow();
    assert_eq!(
        events
            .iter()
            .filter(|(_, p)| p.phase == Phase::Move)
            .count(),
        1
    );
    assert_eq!(events.last().unwrap().1.device, Device::Pen);
}
#[test]
fn actual_touch_contact_ids_do_not_cast_large_native_ids_or_hover() {
    let (mut app, id, log) = fixture();
    let b = bounds_now(&app, id);
    let mut p = pointer((b.x + 11.25) as f32, (b.y + 12.375) as f32, true);
    p.id = apace_traits::events::ApacePointerId::Finger(u64::MAX);
    app.handle_ui_event(UiEvent::PointerMove(p.clone()));
    tick(&mut app);
    assert!(log.borrow().is_empty());
    app.handle_ui_event(UiEvent::PointerDown(p.clone()));
    tick(&mut app);
    let first = log.borrow().last().unwrap().1;
    assert_eq!(first.device, Device::Touch);
    assert!(first.pointer_id >= 2);
    app.handle_ui_event(UiEvent::PointerUp(p));
    tick(&mut app);
    assert_eq!(log.borrow().last().unwrap().1.pointer_id, first.pointer_id);
    assert_eq!(log.borrow().last().unwrap().1.phase, Phase::Up);
}
#[test]
fn nearest_nested_opted_view_is_the_direct_origin() {
    let mut app = WindowApp::new();
    let parent = app.app.scope().cage(None);
    let child = app.app.scope().cage(None);
    let output = Rc::new(RefCell::new(Vec::new()));
    let a = output.clone();
    let b = output.clone();
    app.mount(
        view()
            .node_ref(parent)
            .style(
                Style::new()
                    .width(Length::Px(300.))
                    .height(Length::Px(200.)),
            )
            .on(EventKind::PlotPointer, move |e| {
                a.borrow_mut().push(("parent", e))
            })
            .fill(
                view()
                    .node_ref(child)
                    .style(
                        Style::new()
                            .width(Length::Px(120.25))
                            .height(Length::Px(80.5)),
                    )
                    .on(EventKind::PlotPointer, move |e| {
                        b.borrow_mut().push(("child", e))
                    }),
            ),
    )
    .unwrap();
    tick(&mut app);
    let id = child.get().unwrap();
    let r = bounds_now(&app, id);
    app.handle_ui_event(UiEvent::PointerMove(pointer(
        (r.x + 11.375) as f32,
        (r.y + 9.125) as f32,
        false,
    )));
    tick(&mut app);
    let entries = output.borrow();
    assert!(!entries.is_empty());
    assert!(
        entries
            .iter()
            .all(|(owner, e)| *owner == "child"
                && matches!(e,Event::PlotPointer{from,..}if *from==id))
    );
}
#[test]
fn native_css_transform_ancestors_are_explicitly_outside_first_profile() {
    for css in [
        ".plot{transform:scale(1.25)}",
        ".plot{transform:translate(10px,2px)}",
        "body{transform:rotate(5deg)}",
        ".plot{rotate:5deg}",
        "body{perspective:10px}",
    ] {
        let (mut app, id, log) = fixture();
        app.renderer.set_css(css);
        tick(&mut app);
        let b = bounds_now(&app, id);
        app.handle_ui_event(UiEvent::PointerMove(pointer(
            (b.x + 30.375) as f32,
            (b.y + 30.125) as f32,
            false,
        )));
        tick(&mut app);
        assert!(
            log.borrow().is_empty(),
            "unsupported actual CSS produced a plot callback: {css}"
        );
    }
}
#[test]
fn zero_extent_retires_a_previously_observed_capture_with_terminal_cleanup() {
    let (mut app, id, log) = fixture();
    let b = bounds_now(&app, id);
    app.handle_ui_event(UiEvent::PointerDown(pointer(
        (b.x + 10.375) as f32,
        (b.y + 8.125) as f32,
        true,
    )));
    tick(&mut app);
    assert!(log.borrow().iter().any(|(_, p)| p.phase == Phase::Down));
    app.renderer
        .set_css(".plot{width:0!important;height:0!important}");
    tick(&mut app);
    let events = log.borrow();
    assert_eq!(events.last().unwrap().1.phase, Phase::Cancel);
    assert_eq!(events.last().unwrap().1.viewport, Rect::default());
}
#[test]
fn only_interactive_shell_renderer_advertises_the_channel() {
    assert!(!Renderer::new(RuntimeId::new()).supports_plot_pointer());
    let app = WindowApp::new();
    assert!(app.renderer.supports_plot_pointer());
}

#[test]
fn captured_real_ui_leave_does_not_end_gesture_before_up() {
    let (mut app, id, log) = fixture();
    let b = bounds_now(&app, id);
    let point = pointer((b.x + 9.375) as f32, (b.y + 11.125) as f32, true);
    app.handle_ui_event(UiEvent::PointerDown(point.clone()));
    tick(&mut app);
    log.borrow_mut().clear();
    app.handle_ui_event(UiEvent::PointerLeave(point.clone()));
    tick(&mut app);
    assert!(log.borrow().is_empty());
    app.handle_ui_event(UiEvent::PointerMove(pointer(
        (b.x + b.width + 20.375) as f32,
        (b.y + 11.125) as f32,
        true,
    )));
    tick(&mut app);
    assert_eq!(log.borrow().last().unwrap().1.phase, Phase::Move);
    app.handle_ui_event(UiEvent::PointerUp(pointer(
        (b.x + b.width + 20.375) as f32,
        (b.y + 11.125) as f32,
        false,
    )));
    tick(&mut app);
    let phases = log
        .borrow()
        .iter()
        .map(|(_, p)| p.phase)
        .collect::<Vec<_>>();
    assert_eq!(phases, [Phase::Move, Phase::Up, Phase::Leave]);
}
#[test]
fn old_pointer_interest_and_new_fractional_interest_coexist() {
    let mut app = WindowApp::new();
    let reference = app.app.scope().cage(None);
    let old = Rc::new(RefCell::new(Vec::new()));
    let new = Rc::new(RefCell::new(Vec::new()));
    let a = old.clone();
    let b = new.clone();
    app.mount(
        view()
            .node_ref(reference)
            .style(
                Style::new()
                    .width(Length::Px(200.))
                    .height(Length::Px(100.)),
            )
            .on(EventKind::Pointer, move |e| a.borrow_mut().push(e))
            .on(EventKind::PlotPointer, move |e| b.borrow_mut().push(e)),
    )
    .unwrap();
    tick(&mut app);
    let id = reference.get().unwrap();
    let bounds = bounds_now(&app, id);
    app.handle_ui_event(UiEvent::PointerMove(pointer(
        (bounds.x + 15.375) as f32,
        (bounds.y + 10.125) as f32,
        false,
    )));
    app.handle_ui_event(UiEvent::PointerDown(pointer(
        (bounds.x + 15.375) as f32,
        (bounds.y + 10.125) as f32,
        true,
    )));
    app.handle_ui_event(UiEvent::PointerUp(pointer(
        (bounds.x + 15.375) as f32,
        (bounds.y + 10.125) as f32,
        false,
    )));
    assert!(old.borrow().is_empty());
    assert!(new.borrow().is_empty());
    tick(&mut app);
    assert!(
        old.borrow()
            .iter()
            .any(|e| matches!(e,Event::Pointer{from,..}if *from==id))
    );
    assert!(new.borrow().iter().any(
        |e| matches!(e,Event::PlotPointer{from,pointer}if *from==id&&pointer.phase==Phase::Down)
    ));
}
#[test]
fn inert_and_legal_disabled_fieldset_retire_capture_without_new_moves() {
    for disabled in [false, true] {
        let mut app = WindowApp::new();
        let reference = app.app.scope().cage(None);
        let blocked = app.app.scope().cage(false);
        let log = Rc::new(RefCell::new(Vec::new()));
        let output = log.clone();
        // A disabled structural Fieldset is a real allowed ancestor. A bare
        // View with disabled=true is an invalid declaration, tested separately.
        app.mount(
            view()
                .semantics(blocked.map(move |active| {
                    Some(Box::new(glory_render::Semantics {
                        field_part: disabled.then_some(glory_render::FieldPart::Fieldset),
                        inert: *active && !disabled,
                        disabled: *active && disabled,
                        ..Default::default()
                    }))
                }))
                .fill(
                    view()
                        .node_ref(reference)
                        .style(
                            Style::new()
                                .width(Length::Px(200.))
                                .height(Length::Px(100.)),
                        )
                        .on(EventKind::PlotPointer, move |e| {
                            if let Event::PlotPointer { pointer, .. } = e {
                                output.borrow_mut().push(pointer)
                            }
                        }),
                ),
        )
        .unwrap();
        let observe = |app: &WindowApp| {
            app.observe(&std::task::Context::from_waker(std::task::Waker::noop()))
                .unwrap();
        };
        observe(&app);
        let id = reference.get().unwrap();
        let b = bounds_now(&app, id);
        app.handle_ui_event(UiEvent::PointerDown(pointer(
            (b.x + 15.375) as f32,
            (b.y + 10.125) as f32,
            true,
        )));
        observe(&app);
        assert_eq!(log.borrow().last().unwrap().phase, Phase::Down);
        blocked.revise(|v| *v = true);
        observe(&app); // Do not let WindowApp::poll hide a rejected declaration.
        assert!(!app.app.tree().enabled(id));
        assert_eq!(log.borrow().last().unwrap().phase, Phase::Cancel);
        log.borrow_mut().clear();
        app.handle_ui_event(UiEvent::PointerMove(pointer(
            (b.x + 16.375) as f32,
            (b.y + 11.125) as f32,
            true,
        )));
        observe(&app);
        assert!(log.borrow().is_empty());
        blocked.revise(|v| *v = false);
        observe(&app);
        assert!(app.app.tree().enabled(id));
        app.handle_ui_event(UiEvent::PointerMove(pointer(
            (b.x + 17.375) as f32,
            (b.y + 12.125) as f32,
            false,
        )));
        observe(&app);
        assert_eq!(log.borrow().last().unwrap().phase, Phase::Move);
    }
}

#[test]
fn illegal_bare_view_disabled_patch_rejects_without_retiring_valid_capture() {
    let (mut app, id, log) = fixture();
    let observe = |app: &WindowApp| {
        app.observe(&std::task::Context::from_waker(std::task::Waker::noop()))
            .unwrap();
    };
    let b = bounds_now(&app, id);
    app.handle_ui_event(UiEvent::PointerDown(pointer(
        (b.x + 15.375) as f32,
        (b.y + 10.125) as f32,
        true,
    )));
    observe(&app);
    assert_eq!(log.borrow().last().unwrap().1.phase, Phase::Down);
    let before = app
        .app
        .tree()
        .node(id)
        .unwrap()
        .descriptor
        .view()
        .to_owned();
    let native_before = app
        .renderer
        .0
        .borrow()
        .mapper
        .tree()
        .node(id)
        .unwrap()
        .descriptor
        .view()
        .to_owned();
    let calls = log.borrow().clone();
    let error = app
        .app
        .scope()
        .patch(
            id,
            glory_render::Patch::Semantics(glory_render::Change::Set(Some(Box::new(
                glory_render::Semantics {
                    disabled: true,
                    ..Default::default()
                },
            )))),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        glory_ui::Error::Render(Error::InvalidProps(
            "composite trigger metadata requires a button"
        ))
    ));
    assert_eq!(
        app.app.tree().node(id).unwrap().descriptor.view(),
        before.view()
    );
    assert_eq!(
        app.renderer
            .0
            .borrow()
            .mapper
            .tree()
            .node(id)
            .unwrap()
            .descriptor
            .view(),
        native_before.view()
    );
    app.app.flush().unwrap();
    observe(&app);
    assert_eq!(
        *log.borrow(),
        calls,
        "an unaccepted source does not cancel the accepted lease"
    );
    // The accepted lease still routes outside coordinates and its real Up.
    app.handle_ui_event(UiEvent::PointerMove(pointer(
        (b.x + b.width + 25.25) as f32,
        (b.y + 12.125) as f32,
        true,
    )));
    observe(&app);
    assert_eq!(log.borrow().last().unwrap().1.phase, Phase::Move);
    assert!(log.borrow().last().unwrap().1.x > b.width);
    app.handle_ui_event(UiEvent::PointerUp(pointer(
        (b.x + b.width + 25.25) as f32,
        (b.y + 12.125) as f32,
        false,
    )));
    observe(&app);
    let events = log.borrow();
    assert_eq!(events[events.len() - 2].1.phase, Phase::Up);
    assert_eq!(events.last().unwrap().1.phase, Phase::Leave);
}

#[test]
fn released_capture_still_cancels_if_box_changed_before_observation() {
    let (mut app, id, log) = fixture();
    let b = bounds_now(&app, id);
    app.handle_ui_event(UiEvent::PointerDown(pointer(
        (b.x + 12.375) as f32,
        (b.y + 8.125) as f32,
        true,
    )));
    tick(&mut app);
    log.borrow_mut().clear();
    app.renderer
        .set_css(".plot{width:120px!important;height:90px!important}");
    app.handle_ui_event(UiEvent::PointerUp(pointer(
        (b.x + 12.375) as f32,
        (b.y + 8.125) as f32,
        false,
    )));
    tick(&mut app);
    let values = log.borrow();
    assert!(
        values
            .iter()
            .any(|p| p.1.phase == Phase::Cancel && p.1.viewport == Rect::default())
    );
    assert!(!values.iter().any(|p| p.1.phase == Phase::Up));
}
