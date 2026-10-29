use super::focus_tests::{focus, key, settle};
use super::tests::{bounds_now, pointer};
use super::*;
use glory_render::style::{Length, Style};
use glory_render::{
    AnchorAlign, AnchorPlacement, AnchorSide, DismissReason, FocusMode, Layer, LayerPlacement,
    Role, SemanticId, Semantics,
};
use glory_ui::{button, input, view};

fn panel(
    open: glory_core::Cage<bool>,
    key: &'static str,
    control: glory_core::Cage<Option<NodeId>>,
    modal: bool,
) -> impl glory_ui::Widget {
    view()
        .role(Role::Dialog)
        .name(key)
        .focus(FocusMode::Programmatic)
        .style(
            Style::new()
                .width(Length::Px(240.))
                .height(Length::Px(180.)),
        )
        .semantics(open.map(move |open| {
            Some(Box::new(Semantics {
                id: Some(name(key)),
                hidden: !*open,
                layer: Some(Layer {
                    modal,
                    ..Default::default()
                }),
                ..Default::default()
            }))
        }))
        .fill(input().node_ref(control))
}

fn name(value: &str) -> SemanticId {
    SemanticId::new(value).unwrap()
}
fn focused(app: &WindowApp) -> Option<NodeId> {
    let state = app.renderer.0.borrow();
    state
        .dom
        .doc
        .get_focussed_node_id()
        .and_then(|id| state.dom.owner(id))
}

#[test]
fn native_layer_registry_keeps_ordinary_and_closed_mutations_layout_lazy() {
    use glory_render::{Change, Descriptor, NodeIds, Patch, Props, Render, RuntimeId};
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut renderer = Renderer::new(runtime);
    let ordinary = ids.allocate();
    renderer
        .apply(&[
            Mutation::Create {
                id: ordinary,
                node: Box::new(Descriptor::new(Props::View)),
            },
            Mutation::Place {
                id: ordinary,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    assert_eq!(renderer.stats().layouts, 0);
    let closed = ids.allocate();
    let mut descriptor = Descriptor::new(Props::View);
    descriptor.role = Some(Role::Dialog);
    descriptor.name = Some("Closed dialog".into());
    descriptor.focus = FocusMode::Programmatic;
    descriptor.semantics = Some(Box::new(Semantics {
        id: Some(name("closed-lazy")),
        hidden: true,
        layer: Some(Layer::default()),
        ..Default::default()
    }));
    renderer
        .apply(&[
            Mutation::Create {
                id: closed,
                node: Box::new(descriptor),
            },
            Mutation::Place {
                id: closed,
                parent: None,
                before: None,
            },
            Mutation::Patch {
                id: ordinary,
                patch: Patch::Style(Change::Set(Box::new(Style::new().width(Length::Px(40.))))),
            },
        ])
        .unwrap();
    assert_eq!(
        renderer.stats().layouts,
        0,
        "closed declarations never force layout"
    );
}

#[test]
fn native_modal_layer_initial_tab_escape_controlled_refusal_return_and_cleanup() {
    let mut app = WindowApp::new();
    let open = app.app.scope().cage(false);
    let accept = app.app.scope().cage(false);
    let calls = app.app.scope().cage(Vec::new());
    let trigger = app.app.scope().cage(None);
    let first = app.app.scope().cage(None);
    let second = app.app.scope().cage(None);
    let panel = app.app.scope().cage(None);
    app.mount(
        view().fill((
            button()
                .label("Open")
                .node_ref(trigger)
                .semantics(Some(Box::new(Semantics {
                    id: Some(name("trigger")),
                    ..Default::default()
                }))),
            view()
                .role(Role::Dialog)
                .name("Native dialog")
                .focus(FocusMode::Programmatic)
                .node_ref(panel)
                .style(
                    Style::new()
                        .width(Length::Px(240.))
                        .height(Length::Px(160.)),
                )
                .semantics(open.map(|open| {
                    Some(Box::new(Semantics {
                        id: Some(name("panel")),
                        hidden: !*open,
                        layer: Some(Layer {
                            initial_focus: Some(name("second")),
                            return_focus: Some(name("trigger")),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .on(EventKind::Dismiss, move |event| {
                    if let Event::Dismiss(reason) = event {
                        calls.revise(|values| values.push(reason));
                        if accept.get() {
                            open.revise(|open| *open = false);
                        }
                    }
                })
                .fill((
                    input().node_ref(first),
                    input().node_ref(second).semantics(Some(Box::new(Semantics {
                        id: Some(name("second")),
                        ..Default::default()
                    }))),
                )),
        )),
    )
    .unwrap();
    settle(&mut app);
    focus(&mut app, trigger.get().unwrap());
    open.revise(|v| *v = true);
    settle(&mut app);
    assert_eq!(focused(&app), second.get());
    let rect = bounds_now(&app, panel.get().unwrap());
    assert_eq!(
        (rect.x, rect.y, rect.width, rect.height),
        (280., 220., 240., 160.)
    );
    assert_eq!(app.renderer.inspect(|doc| doc.top_layers().len()), 1);
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), first.get());
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::SHIFT);
    assert_eq!(focused(&app), second.get());
    assert!(
        app.renderer
            .clone()
            .action(glory_render::Action::Focus(trigger.get().unwrap()))
            .is_err()
    );
    key(&mut app, Key::Named(NamedKey::Escape), Modifiers::empty());
    assert_eq!(calls.get(), vec![DismissReason::Escape]);
    assert!(open.get());
    assert_eq!(focused(&app), second.get());
    app.handle_ui_event(UiEvent::PointerDown(pointer(10., 10., true)));
    app.handle_ui_event(UiEvent::PointerUp(pointer(10., 10., false)));
    settle(&mut app);
    assert_eq!(
        calls.get(),
        vec![DismissReason::Escape, DismissReason::Outside]
    );
    assert_eq!(focused(&app), second.get());
    accept.revise(|value| *value = true);
    key(&mut app, Key::Named(NamedKey::Escape), Modifiers::empty());
    assert!(!open.get());
    assert_eq!(focused(&app), trigger.get());
    assert!(app.renderer.inspect(|doc| doc.top_layers().is_empty()));
    app.app.clear().unwrap();
    settle(&mut app);
    assert!(app.renderer.0.borrow().layers.open.is_empty());
}

#[test]
fn native_anchored_layer_uses_all_finite_sides_and_survives_viewport_scroll() {
    let mut app = WindowApp::new();
    let policy = app.app.scope().cage(AnchorPlacement::default());
    let anchor = app.app.scope().cage(None);
    let panel = app.app.scope().cage(None);
    app.mount(
        view().fill((
            button()
                .label("Anchor")
                .node_ref(anchor)
                .semantics(Some(Box::new(Semantics {
                    id: Some(name("anchor")),
                    ..Default::default()
                })))
                .style(
                    Style::new()
                        .position(glory_render::style::Position::Absolute)
                        .left(Length::Px(300.))
                        .top(Length::Px(250.))
                        .width(Length::Px(80.))
                        .height(Length::Px(30.)),
                ),
            view()
                .role(Role::Dialog)
                .name("Popover")
                .focus(FocusMode::Programmatic)
                .node_ref(panel)
                .style(Style::new().width(Length::Px(120.)).height(Length::Px(70.)))
                .semantics(policy.map(|policy| {
                    Some(Box::new(Semantics {
                        id: Some(name("popover")),
                        layer: Some(Layer {
                            modal: false,
                            placement: LayerPlacement::Anchor(*policy),
                            anchor: Some(name("anchor")),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .fill(input()),
        )),
    )
    .unwrap();
    settle(&mut app);
    for side in [
        AnchorSide::Top,
        AnchorSide::Bottom,
        AnchorSide::Left,
        AnchorSide::Right,
    ] {
        for align in [AnchorAlign::Start, AnchorAlign::Center, AnchorAlign::End] {
            let next = AnchorPlacement::new(side, align).gap(13.);
            policy.revise(|value| *value = next);
            settle(&mut app);
            let a = bounds_now(&app, anchor.get().unwrap());
            let p = bounds_now(&app, panel.get().unwrap());
            let expected = next
                .resolve(
                    a,
                    p,
                    Rect {
                        x: 0.,
                        y: 0.,
                        width: 800.,
                        height: 600.,
                    },
                )
                .unwrap();
            assert_eq!((p.x, p.y), (expected.x, expected.y), "{side:?}/{align:?}");
        }
    }
}

#[test]
fn native_viewport_layer_resolves_live_coordinates_without_replacing_its_editor() {
    let mut app = WindowApp::new();
    let position = app
        .app
        .scope()
        .cage(glory_render::ViewportPlacement::default());
    let panel = app.app.scope().cage(None);
    let editor = app.app.scope().cage(None);
    app.mount(
        view()
            .role(Role::Dialog)
            .name("Viewport window")
            .focus(FocusMode::Programmatic)
            .node_ref(panel)
            .style(
                Style::new()
                    .width(Length::Px(240.))
                    .height(Length::Px(180.)),
            )
            .semantics(position.map(|placement| {
                Some(Box::new(Semantics {
                    id: Some(name("native-window")),
                    layer: Some(Layer {
                        modal: false,
                        outside: false,
                        placement: LayerPlacement::Viewport(*placement),
                        ..Default::default()
                    }),
                    ..Default::default()
                }))
            }))
            .fill(input().value("stable editor").node_ref(editor)),
    )
    .unwrap();
    settle(&mut app);
    let panel = panel.get().unwrap();
    let engine = app
        .renderer
        .engine_id(editor.get().unwrap(), Part::Main)
        .unwrap();
    assert_eq!(
        (bounds_now(&app, panel).x, bounds_now(&app, panel).y),
        (48., 48.)
    );
    position.revise(|p| {
        p.x = 10000.;
        p.y = -100.;
    });
    settle(&mut app);
    let clamped = bounds_now(&app, panel);
    assert_eq!((clamped.x, clamped.y), (552., 8.));
    position.revise(|p| {
        p.x = 135.5;
        p.y = 225.25;
        p.constrain = false;
    });
    settle(&mut app);
    let moved = bounds_now(&app, panel);
    assert_eq!((moved.x, moved.y), (135.5, 225.25));
    assert_eq!(
        app.renderer
            .engine_id(editor.get().unwrap(), Part::Main)
            .unwrap(),
        engine
    );
    assert_eq!(focused(&app), editor.get());
}

#[test]
fn native_raise_preserves_actual_editor_selection_preedit_and_rejects_modal_crossing() {
    let mut app = WindowApp::new();
    let first = app.app.scope().cage(None);
    let second = app.app.scope().cage(None);
    let editor = app.app.scope().cage(None);
    let modal = app.app.scope().cage(false);
    let modal_editor = app.app.scope().cage(None);
    let window = |key: &'static str, reference, content| {
        view()
            .role(Role::Dialog)
            .name(key)
            .focus(FocusMode::Programmatic)
            .node_ref(reference)
            .style(
                Style::new()
                    .width(Length::Px(260.))
                    .height(Length::Px(180.)),
            )
            .semantics(Some(Box::new(Semantics {
                id: Some(name(key)),
                layer: Some(Layer {
                    modal: false,
                    outside: false,
                    ..Default::default()
                }),
                ..Default::default()
            })))
            .fill(content)
    };
    app.mount(view().fill((
        window("window-a", first, input().value("draft").node_ref(editor)),
        window("window-b", second, input()),
        panel(modal, "blocking-modal", modal_editor, true),
    )))
    .unwrap();
    settle(&mut app);
    let first = first.get().unwrap();
    let editor = editor.get().unwrap();
    let physical = app.renderer.engine_id(editor, Part::Main).unwrap();
    focus(&mut app, editor);
    app.handle_ui_event(UiEvent::Ime(ApaceImeEvent::Preedit(
        "中".into(),
        Some((3, 3)),
    )));
    settle(&mut app);
    let before = (
        super::focus_tests::selection(&mut app, editor),
        app.renderer.0.borrow().dom.text_value(editor).unwrap(),
    );
    app.renderer
        .action(glory_render::Action::RaiseLayer(first))
        .unwrap();
    settle(&mut app);
    assert_eq!(
        app.renderer.engine_id(editor, Part::Main).unwrap(),
        physical
    );
    assert_eq!(focused(&app), Some(editor));
    assert_eq!(
        (
            super::focus_tests::selection(&mut app, editor),
            app.renderer.0.borrow().dom.text_value(editor).unwrap()
        ),
        before
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
    let main = app.renderer.engine_id(first, Part::Main).unwrap();
    assert_eq!(
        app.renderer
            .inspect(|doc| doc.top_layers().last().unwrap().node_id),
        main
    );
    modal.revise(|value| *value = true);
    settle(&mut app);
    assert!(matches!(
        app.renderer.action(glory_render::Action::RaiseLayer(first)),
        Err(Error::Unsupported(_))
    ));
    assert_eq!(focused(&app), modal_editor.get());
}

#[test]
fn native_scroll_into_view_uses_current_ancestors_preserves_editor_focus_and_stops_at_the_layer() {
    use glory_render::{Action, style::Overflow};
    let mut app = WindowApp::new();
    let outside = app.app.scope().cage(None);
    let target = app.app.scope().cage(None);
    let scroller = app.app.scope().cage(None);
    let panel = app.app.scope().cage(None);
    app.mount(
        view().fill((
            input().value("caret").node_ref(outside),
            view().style(Style::new().height(Length::Px(1400.))),
            view()
                .role(Role::Dialog)
                .name("Scroll scope")
                .focus(FocusMode::Programmatic)
                .node_ref(panel)
                .style(
                    Style::new()
                        .width(Length::Px(300.))
                        .height(Length::Px(200.)),
                )
                .semantics(Some(Box::new(Semantics {
                    id: Some(name("scroll-scope")),
                    layer: Some(Layer {
                        modal: false,
                        outside: false,
                        ..Default::default()
                    }),
                    ..Default::default()
                })))
                .fill(
                    view()
                        .node_ref(scroller)
                        .style(
                            Style::new()
                                .height(Length::Px(100.))
                                .overflow_y(Overflow::Auto),
                        )
                        .fill((
                            view().style(Style::new().height(Length::Px(600.))),
                            button().label("Reveal current box").node_ref(target),
                        )),
                ),
        )),
    )
    .unwrap();
    settle(&mut app);
    let outside = outside.get().unwrap();
    let target = target.get().unwrap();
    let scroller = scroller.get().unwrap();
    focus(&mut app, outside);
    app.handle_ui_event(UiEvent::Ime(ApaceImeEvent::Preedit(
        "中".into(),
        Some((3, 3)),
    )));
    settle(&mut app);
    {
        let mut state = app.renderer.0.borrow_mut();
        state
            .dom
            .doc
            .set_viewport_scroll(apace_dom::Point { x: 0., y: 240. });
        state.dom.invalidate();
    }
    settle(&mut app);
    let before = (
        super::focus_tests::selection(&mut app, outside),
        app.renderer.0.borrow().dom.text_value(outside).unwrap(),
    );
    let applied = app.renderer.stats().applied;
    app.app
        .scope()
        .action(Action::ScrollIntoView(target))
        .unwrap();
    app.app.flush().unwrap();
    settle(&mut app);
    let rect = bounds_now(&app, target);
    let outer = bounds_now(&app, scroller);
    assert!(
        rect.y >= outer.y - 0.1 && rect.y + rect.height <= outer.y + outer.height + 0.1,
        "{rect:?} {outer:?}"
    );
    assert_eq!(app.renderer.inspect(|doc| doc.viewport_scroll().y), 240.);
    assert_eq!(app.renderer.stats().applied, applied);
    assert_eq!(focused(&app), Some(outside));
    assert_eq!(
        (
            super::focus_tests::selection(&mut app, outside),
            app.renderer.0.borrow().dom.text_value(outside).unwrap()
        ),
        before
    );
    assert!(
        app.renderer
            .0
            .borrow()
            .dom
            .range(outside)
            .unwrap()
            .editing
            .composing
    );
    app.renderer
        .action(Action::ScrollIntoView(panel.get().unwrap()))
        .unwrap();
    assert_eq!(app.renderer.inspect(|doc| doc.viewport_scroll().y), 240.);
}

#[test]
fn native_nested_and_out_of_order_modals_preserve_current_focus_and_transfer_fallbacks() {
    let mut app = WindowApp::new();
    let a = app.app.scope().cage(false);
    let b = app.app.scope().cage(false);
    let trigger = app.app.scope().cage(None);
    let first = app.app.scope().cage(None);
    let second = app.app.scope().cage(None);
    app.mount(view().fill((
        button().label("Original").node_ref(trigger),
        panel(a, "outer-modal", first, true),
        panel(b, "upper-modal", second, true),
    )))
    .unwrap();
    settle(&mut app);
    focus(&mut app, trigger.get().unwrap());
    a.revise(|v| *v = true);
    settle(&mut app);
    assert_eq!(focused(&app), first.get());
    b.revise(|v| *v = true);
    settle(&mut app);
    assert_eq!(focused(&app), second.get());
    a.revise(|v| *v = false);
    settle(&mut app);
    assert_eq!(
        focused(&app),
        second.get(),
        "closing the lower modal must preserve upper focus"
    );
    b.revise(|v| *v = false);
    settle(&mut app);
    assert_eq!(
        focused(&app),
        trigger.get(),
        "no retired lower layer is retained for fallback"
    );
    assert!(app.renderer.0.borrow().layers.open.is_empty());
    a.revise(|v| *v = true);
    settle(&mut app);
    b.revise(|v| *v = true);
    settle(&mut app);
    b.revise(|v| *v = false);
    settle(&mut app);
    assert_eq!(focused(&app), first.get());
}

#[test]
fn native_nonmodal_outside_pointer_preserves_clicked_focus_and_tab_leaves_the_scope() {
    let mut app = WindowApp::new();
    let open = app.app.scope().cage(true);
    let outside = app.app.scope().cage(None);
    let inner = app.app.scope().cage(None);
    let reasons = app.app.scope().cage(Vec::new());
    app.mount(
        view().fill((
            button()
                .label("Outside")
                .node_ref(outside)
                .style(Style::new().width(Length::Px(120.)).height(Length::Px(30.))),
            view()
                .role(Role::Dialog)
                .name("Nonmodal")
                .focus(FocusMode::Programmatic)
                .style(
                    Style::new()
                        .width(Length::Px(240.))
                        .height(Length::Px(180.)),
                )
                .semantics(open.map(|open| {
                    Some(Box::new(Semantics {
                        id: Some(name("nonmodal")),
                        hidden: !*open,
                        layer: Some(Layer {
                            modal: false,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .on(EventKind::Dismiss, move |event| {
                    if let Event::Dismiss(reason) = event {
                        reasons.revise(|v| v.push(reason));
                        open.revise(|v| *v = false);
                    }
                })
                .fill(input().node_ref(inner)),
        )),
    )
    .unwrap();
    settle(&mut app);
    assert_eq!(focused(&app), inner.get());
    let rect = bounds_now(&app, outside.get().unwrap());
    let x = (rect.x + 10.) as f32;
    let y = (rect.y + 10.) as f32;
    app.handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    app.handle_ui_event(UiEvent::PointerUp(pointer(x, y, false)));
    settle(&mut app);
    assert_eq!(reasons.get(), vec![DismissReason::Outside]);
    assert_eq!(focused(&app), outside.get());
    open.revise(|v| *v = true);
    settle(&mut app);
    assert_eq!(focused(&app), inner.get());
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), outside.get());
    assert!(!open.get());
}

#[test]
fn native_modal_wheel_scrolls_its_viewport_and_never_chains_to_the_document() {
    use apace_traits::events::{ApaceWheelDelta, ApaceWheelEvent};
    let mut app = WindowApp::new();
    let open = app.app.scope().cage(false);
    let panel_ref = app.app.scope().cage(None);
    let editor = app.app.scope().cage(None);
    app.mount(
        view().style(Style::new().height(Length::Px(1600.))).fill(
            view()
                .role(Role::Dialog)
                .name("Scroll modal")
                .focus(FocusMode::Programmatic)
                .node_ref(panel_ref)
                .style(
                    Style::new()
                        .width(Length::Px(260.))
                        .height(Length::Px(220.))
                        .overflow_y(glory_render::style::Overflow::Auto),
                )
                .semantics(open.map(|open| {
                    Some(Box::new(Semantics {
                        id: Some(name("scroll-modal")),
                        hidden: !*open,
                        layer: Some(Layer::default()),
                        ..Default::default()
                    }))
                }))
                .fill(
                    view()
                        .style(Style::new().height(Length::Px(900.)))
                        .fill(input().node_ref(editor)),
                ),
        ),
    )
    .unwrap();
    settle(&mut app);
    {
        let mut state = app.renderer.0.borrow_mut();
        state
            .dom
            .doc
            .set_viewport_scroll(apace_dom::Point { x: 0., y: 240. });
        state.dom.invalidate();
    }
    open.revise(|v| *v = true);
    settle(&mut app);
    let rect = bounds_now(&app, editor.get().unwrap());
    let coords = pointer((rect.x + 20.) as f32, (rect.y + 10.) as f32, false).coords;
    let mut hover = pointer((rect.x + 20.) as f32, (rect.y + 10.) as f32, false);
    hover.coords.page_y += 240.;
    app.handle_ui_event(UiEvent::PointerMove(hover));
    for _ in 0..5 {
        app.handle_ui_event(UiEvent::Wheel(ApaceWheelEvent {
            delta: ApaceWheelDelta::Pixels(0., -120.),
            coords,
            buttons: Default::default(),
            mods: Default::default(),
            element: Default::default(),
        }));
        settle(&mut app);
    }
    let state = app.renderer.0.borrow();
    let physical = state.dom.main(panel_ref.get().unwrap()).unwrap();
    assert!(state.dom.doc.get_node(physical).unwrap().scroll_offset().y > 0.);
    assert_eq!(state.dom.doc.viewport_scroll().y, 240.);
    drop(state);
    open.revise(|v| *v = false);
    settle(&mut app);
    assert_eq!(app.renderer.inspect(|doc| doc.viewport_scroll().y), 240.);
}

#[test]
fn native_preserve_layer_does_not_autofocus_trap_tab_or_restore_focus() {
    let mut app = WindowApp::new();
    let open = app.app.scope().cage(false);
    let outside = app.app.scope().cage(None);
    let inner = app.app.scope().cage(None);
    let calls = app.app.scope().cage(0usize);
    app.mount(
        view().fill((
            input().node_ref(outside),
            view()
                .role(Role::Dialog)
                .name("Preserve notice")
                .focus(FocusMode::Programmatic)
                .style(
                    Style::new()
                        .width(Length::Px(240.))
                        .height(Length::Px(180.)),
                )
                .semantics(open.map(|open| {
                    Some(Box::new(Semantics {
                        id: Some(name("preserve-notice")),
                        hidden: !*open,
                        layer: Some(Layer {
                            modal: false,
                            focus: glory_render::LayerFocus::Preserve,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .on(EventKind::Dismiss, move |event| {
                    if matches!(event, Event::Dismiss(_)) {
                        calls.revise(|v| *v += 1);
                        open.revise(|v| *v = false);
                    }
                })
                .fill(input().node_ref(inner)),
        )),
    )
    .unwrap();
    settle(&mut app);
    focus(&mut app, outside.get().unwrap());
    open.revise(|v| *v = true);
    settle(&mut app);
    assert_eq!(focused(&app), outside.get());
    key(&mut app, Key::Named(NamedKey::Escape), Modifiers::empty());
    assert_eq!(calls.get(), 0);
    assert!(open.get());
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), inner.get());
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), outside.get());
    assert!(open.get());
    focus(&mut app, inner.get().unwrap());
    key(&mut app, Key::Named(NamedKey::Escape), Modifiers::empty());
    assert!(!open.get());
    assert_eq!(calls.get(), 1);
    assert_ne!(focused(&app), outside.get());
}

#[test]
fn native_modal_suppresses_unrelated_popup_but_admits_a_live_anchor_scope() {
    let mut app = WindowApp::new();
    let modal = app.app.scope().cage(false);
    let popup = app.app.scope().cage(false);
    let anchor = app.app.scope().cage(None);
    let inner = app.app.scope().cage(None);
    let related = app.app.scope().cage(false);
    let extra = app.app.scope().cage(None);
    app.mount(
        view().fill((
            view()
                .role(Role::Dialog)
                .name("Modal")
                .focus(FocusMode::Programmatic)
                .style(
                    Style::new()
                        .width(Length::Px(240.))
                        .height(Length::Px(180.)),
                )
                .semantics(modal.map(|open| {
                    Some(Box::new(Semantics {
                        id: Some(name("scope-modal")),
                        hidden: !*open,
                        layer: Some(Layer::default()),
                        ..Default::default()
                    }))
                }))
                .fill(input().node_ref(anchor).semantics(Some(Box::new(Semantics {
                    id: Some(name("scope-anchor")),
                    ..Default::default()
                })))),
            view()
                .role(Role::Dialog)
                .name("External popup")
                .focus(FocusMode::Programmatic)
                .style(
                    Style::new()
                        .width(Length::Px(140.))
                        .height(Length::Px(100.)),
                )
                .semantics(popup.map(move |open| {
                    let related = related.get();
                    Some(Box::new(Semantics {
                        id: Some(name("external-popup")),
                        hidden: !*open,
                        layer: Some(Layer {
                            modal: false,
                            anchor: related.then(|| name("scope-anchor")),
                            placement: if related {
                                LayerPlacement::Anchor(AnchorPlacement::default())
                            } else {
                                LayerPlacement::Center
                            },
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .fill((input().node_ref(inner), input().node_ref(extra))),
        )),
    )
    .unwrap();
    settle(&mut app);
    modal.revise(|v| *v = true);
    settle(&mut app);
    popup.revise(|v| *v = true);
    settle(&mut app);
    assert_eq!(focused(&app), anchor.get());
    assert_eq!(app.renderer.inspect(|doc| doc.top_layers().len()), 1);
    related.revise(|v| *v = true);
    settle(&mut app);
    assert_eq!(app.renderer.inspect(|doc| doc.top_layers().len()), 2);
    focus(&mut app, inner.get().unwrap());
    assert_eq!(focused(&app), inner.get());
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), extra.get());
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(
        focused(&app),
        anchor.get(),
        "nonmodal popup does not trap focus inside the modal"
    );
}
