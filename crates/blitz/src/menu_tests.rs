use super::focus_tests::{focus, key, settle};
use super::tests::pointer;
use super::*;
use glory_render::{
    Action, Change, ContextEditPolicy, ContextPoint, ContextSource, Direction, FocusMode, Layer,
    LayerFocus, LayerPlacement, MenuCommand, MenuItemKind, MenuItemSemantics, Orientation, Patch,
    PopupKind, Role, SemanticId, Semantics,
};
use glory_ui::{button, check, group, input, select, view};
fn name(s: &str) -> SemanticId {
    SemanticId::new(s).unwrap()
}
fn menu_metadata(id: &str, trigger: &str, hidden: bool) -> Option<Box<Semantics>> {
    Some(Box::new(Semantics {
        id: Some(name(id)),
        navigation: Some(Orientation::Vertical),
        typeahead: true,
        hidden,
        layer: Some(Layer {
            modal: false,
            focus: LayerFocus::Manage,
            placement: LayerPlacement::Anchor(glory_render::AnchorPlacement::new(
                glory_render::AnchorSide::Bottom,
                glory_render::AnchorAlign::Start,
            )),
            anchor: Some(name(trigger)),
            return_focus: Some(name(trigger)),
            ..Default::default()
        }),
        ..Default::default()
    }))
}
fn member(kind: MenuItemKind) -> Option<Box<Semantics>> {
    Some(Box::new(Semantics {
        navigation_item: true,
        menu_item: Some(MenuItemSemantics {
            kind,
            owner: name("panel"),
        }),
        ..Default::default()
    }))
}
#[test]
fn native_menu_trigger_submenu_rtl_typeahead_and_check_enter_use_live_component_protocols() {
    let mut host = WindowApp::new();
    let scope = host.app.scope();
    let trigger = scope.cage(None);
    let item = scope.cage(None);
    let checkbox = scope.cage(None);
    let commands = Rc::new(RefCell::new(Vec::new()));
    let searches = Rc::new(RefCell::new(Vec::new()));
    let output = commands.clone();
    let root_output = commands.clone();
    let search = searches.clone();
    let checked = scope.cage(false);
    let changes = scope.cage(0usize);
    host.mount(
        group().fill((
            button()
                .label("Open")
                .node_ref(trigger)
                .semantics(Some(Box::new(Semantics {
                    id: Some(name("trigger")),
                    popup: Some(PopupKind::Menu),
                    expanded: Some(true),
                    ..Default::default()
                })))
                .on(EventKind::Menu, move |event| {
                    if let Event::Menu { from, command } = event {
                        output.borrow_mut().push((from, command))
                    }
                }),
            view()
                .role(Role::Menu)
                .name("Actions")
                .focus(FocusMode::Programmatic)
                .semantics({
                    let mut s = menu_metadata("panel", "trigger", false);
                    s.as_mut().unwrap().navigation_direction = Direction::Rtl;
                    s
                })
                .on(EventKind::Menu, move |event| {
                    if let Event::Menu { from, command } = event {
                        root_output.borrow_mut().push((from, command))
                    }
                })
                .on(EventKind::Typeahead, move |event| {
                    if let Event::Typeahead { from, text, .. } = event {
                        search.borrow_mut().push((from, text))
                    }
                })
                .fill((
                    button()
                        .label("Submenu")
                        .focus(FocusMode::TabStop)
                        .node_ref(item)
                        .semantics(member(MenuItemKind::Action)),
                    check()
                        .label("Toggle")
                        .focus(FocusMode::Programmatic)
                        .node_ref(checkbox)
                        .semantics(member(MenuItemKind::Checkbox))
                        .checked(checked)
                        .on(EventKind::CheckedChanged, move |event| {
                            if let Event::CheckedChanged(next) = event {
                                checked.revise(|v| *v = next);
                                changes.revise(|v| *v += 1)
                            }
                        }),
                )),
        )),
    )
    .unwrap();
    let trigger = trigger.get().unwrap();
    let item = item.get().unwrap();
    let checkbox = checkbox.get().unwrap();
    focus(&mut host, trigger);
    key(
        &mut host,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
    );
    key(&mut host, Key::Named(NamedKey::ArrowUp), Modifiers::empty());
    focus(&mut host, item);
    key(
        &mut host,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::empty(),
    );
    key(
        &mut host,
        Key::Named(NamedKey::ArrowRight),
        Modifiers::empty(),
    );
    assert_eq!(
        *commands.borrow(),
        [
            (trigger, MenuCommand::First),
            (trigger, MenuCommand::Last),
            (item, MenuCommand::Open),
            (item, MenuCommand::Close)
        ]
    );
    key(&mut host, Key::Character("中".into()), Modifiers::empty());
    assert_eq!(*searches.borrow(), [(item, "中".into())]);
    key(
        &mut host,
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::CONTROL,
    );
    assert_eq!(commands.borrow().len(), 4);
    focus(&mut host, checkbox);
    key(&mut host, Key::Named(NamedKey::Enter), Modifiers::empty());
    assert!(checked.get());
    assert_eq!(changes.get(), 1);
    host.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
        key: Key::Named(NamedKey::Enter),
        code: keyboard_types::Code::Enter,
        modifiers: Modifiers::empty(),
        location: keyboard_types::Location::Standard,
        is_auto_repeating: true,
        is_composing: false,
        state: blitz_traits::events::KeyState::Pressed,
        text: None,
    }));
    settle(&mut host);
    assert!(checked.get());
    assert_eq!(changes.get(), 1);
}
#[test]
fn native_palette_nearest_keyboard_interest_wins_and_editors_keep_native_chords() {
    let mut host = WindowApp::new();
    let scope = host.app.scope();
    let target = scope.cage(None);
    let editor = scope.cage(None);
    let intrinsic = scope.cage(None);
    let nearer = scope.cage(None);
    let palette = scope.cage(0usize);
    let keyboard = scope.cage(0usize);
    host.mount(
        view()
            .on(EventKind::Palette, move |_| palette.revise(|v| *v += 1))
            .fill((
                button().label("Shortcut target").node_ref(target),
                input().value("Keep text").node_ref(editor),
                select().node_ref(intrinsic),
                view()
                    .on(EventKind::Keyboard, move |_| keyboard.revise(|v| *v += 1))
                    .fill(button().label("Nearest keyboard").node_ref(nearer)),
            )),
    )
    .unwrap();
    for (id, expected) in [
        (target.get().unwrap(), 1),
        (editor.get().unwrap(), 1),
        (intrinsic.get().unwrap(), 1),
        (nearer.get().unwrap(), 1),
    ] {
        focus(&mut host, id);
        key(&mut host, Key::Character("k".into()), Modifiers::CONTROL);
        assert_eq!(palette.get(), expected);
    }
    assert_eq!(keyboard.get(), 1);
    focus(&mut host, target.get().unwrap());
    key(&mut host, Key::Character("k".into()), Modifiers::META);
    assert_eq!(palette.get(), 2);
    key(
        &mut host,
        Key::Character("k".into()),
        Modifiers::CONTROL | Modifiers::SHIFT,
    );
    assert_eq!(palette.get(), 2);
    host.app.clear().unwrap();
    key(&mut host, Key::Character("k".into()), Modifiers::CONTROL);
    assert_eq!(palette.get(), 2);
}
#[test]
fn native_context_nearest_region_editor_policy_pointer_coordinates_and_keyboard_anchor() {
    let mut host = WindowApp::new();
    host.renderer.set_viewport(800, 600, 2.).unwrap();
    let scope = host.app.scope();
    let region = scope.cage(None);
    let target = scope.cage(None);
    let edit = scope.cage(None);
    let inner = scope.cage(None);
    let nested = scope.cage(None);
    let received = Rc::new(RefCell::new(Vec::new()));
    let output = received.clone();
    let meta = |id: &str, policy| {
        Some(Box::new(Semantics {
            id: Some(name(id)),
            context_menu: Some(policy),
            ..Default::default()
        }))
    };
    host.mount(
        view()
            .role(Role::Group)
            .focus(FocusMode::TabStop)
            .node_ref(region)
            .semantics(meta("area", ContextEditPolicy::Native))
            .on(EventKind::ContextMenu, move |event| {
                if let Event::ContextMenu {
                    from,
                    point,
                    source,
                } = event
                {
                    output.borrow_mut().push((from, point, source))
                }
            })
            .fill((
                button().label("Context target").node_ref(target),
                input().value("Native editor").node_ref(edit),
                view()
                    .role(Role::Group)
                    .focus(FocusMode::TabStop)
                    .node_ref(inner)
                    .semantics(meta("inner", ContextEditPolicy::Override))
                    .fill(button().label("No inner subscription").node_ref(nested)),
            )),
    )
    .unwrap();
    let target = target.get().unwrap();
    let edit = edit.get().unwrap();
    let nested = nested.get().unwrap();
    let send = |host: &mut WindowApp, id, kind| {
        let physical = host.renderer.engine_id(id, Part::Main).unwrap();
        let mut p = pointer(40., 50., false);
        p.id = kind;
        let events = Events::new(host.renderer.clone());
        EventDriver::new(host, events)
            .handle_dom_event(DomEvent::new(physical, DomEventData::ContextMenu(p)));
        settle(host);
    };
    send(
        &mut host,
        target,
        blitz_traits::events::BlitzPointerId::Mouse,
    );
    assert_eq!(
        *received.borrow(),
        [(
            target,
            ContextPoint { x: 40., y: 50. },
            ContextSource::Pointer
        )]
    );
    send(&mut host, edit, blitz_traits::events::BlitzPointerId::Mouse);
    send(
        &mut host,
        nested,
        blitz_traits::events::BlitzPointerId::Mouse,
    );
    send(
        &mut host,
        target,
        blitz_traits::events::BlitzPointerId::Finger(3),
    );
    assert_eq!(received.borrow().len(), 1);
    host.app
        .scope()
        .patch(
            region.get().unwrap(),
            Patch::Semantics(Change::Set(meta("area", ContextEditPolicy::Override))),
        )
        .unwrap();
    host.app.flush().unwrap();
    send(&mut host, edit, blitz_traits::events::BlitzPointerId::Pen);
    assert_eq!(received.borrow().len(), 2);
    focus(&mut host, target);
    key(
        &mut host,
        Key::Named(NamedKey::ContextMenu),
        Modifiers::empty(),
    );
    let expected = {
        let mut state = host.renderer.0.borrow_mut();
        state.dom.resolve();
        let n = state.dom.control(target).unwrap();
        let r = state.dom.doc.get_client_bounding_rect(n).unwrap();
        ContextPoint {
            x: r.x,
            y: r.y + r.height,
        }
    };
    assert_eq!(
        received.borrow().last().unwrap(),
        &(target, expected, ContextSource::Keyboard)
    );
    key(&mut host, Key::Named(NamedKey::F10), Modifiers::SHIFT);
    assert_eq!(received.borrow().len(), 4);
    host.app
        .scope()
        .patch(
            region.get().unwrap(),
            Patch::Semantics(Change::Set({
                let mut s = meta("area", ContextEditPolicy::Override);
                s.as_mut().unwrap().disabled = true;
                s
            })),
        )
        .unwrap();
    host.app.flush().unwrap();
    send(
        &mut host,
        target,
        blitz_traits::events::BlitzPointerId::Mouse,
    );
    assert_eq!(received.borrow().len(), 4);
}
#[test]
fn native_menu_exit_accepts_or_refuses_close_without_trapping_tab_and_skips_all_menu_trees() {
    for (backwards, accept) in [(false, true), (true, true), (false, false)] {
        let mut host = WindowApp::new();
        let scope = host.app.scope();
        let trigger = scope.cage(None);
        let first = scope.cage(None);
        let after = scope.cage(None);
        let before = scope.cage(None);
        let panel = scope.cage(None);
        let open = scope.cage(true);
        let handler = scope.clone();
        host.mount(
            view().fill((
                button().label("Before").node_ref(before),
                button()
                    .label("Trigger")
                    .node_ref(trigger)
                    .semantics(Some(Box::new(Semantics {
                        id: Some(name("trigger")),
                        ..Default::default()
                    }))),
                view()
                    .role(Role::Menu)
                    .name("Menu")
                    .node_ref(panel)
                    .focus(FocusMode::Programmatic)
                    .semantics(open.map(|open| menu_metadata("panel", "trigger", !*open)))
                    .on(EventKind::Menu, move |event| {
                        if let Event::Menu {
                            command: MenuCommand::Exit { backwards },
                            ..
                        } = event
                        {
                            if accept {
                                open.revise(|v| *v = false)
                            }
                            handler
                                .action(Action::ExitMenu {
                                    node: panel.get().unwrap(),
                                    backwards,
                                })
                                .unwrap();
                        }
                    })
                    .fill(
                        button()
                            .label("Inside")
                            .focus(FocusMode::TabStop)
                            .node_ref(first)
                            .semantics(member(MenuItemKind::Action)),
                    ),
                button().label("After").node_ref(after),
            )),
        )
        .unwrap();
        focus(&mut host, first.get().unwrap());
        key(
            &mut host,
            Key::Named(NamedKey::Tab),
            if backwards {
                Modifiers::SHIFT
            } else {
                Modifiers::empty()
            },
        );
        assert_eq!(
            host.renderer.0.borrow().focused,
            Some(if backwards {
                before.get().unwrap()
            } else {
                after.get().unwrap()
            })
        );
        assert_eq!(open.get(), !accept);
    }
}

pub(super) struct ModalFixture {
    pub host: WindowApp,
    pub panel: NodeId,
    pub item: NodeId,
    pub trigger: NodeId,
    pub after: NodeId,
    pub background: NodeId,
    pub open: glory_core::Cage<bool>,
    pub picked: glory_core::Cage<usize>,
}
pub(super) fn modal_fixture(scale: f32) -> ModalFixture {
    use glory_render::style::{Color, Length, Style};
    let host = WindowApp::new();
    host.renderer
        .set_viewport((640. * scale) as u32, (480. * scale) as u32, scale)
        .unwrap();
    host.renderer
        .set_css("body{background:white}button{height:30px}");
    let scope = host.app.scope();
    let trigger = scope.cage(None);
    let item = scope.cage(None);
    let after = scope.cage(None);
    let background = scope.cage(None);
    let panel = scope.cage(None);
    let open = scope.cage(true);
    let picked = scope.cage(0usize);
    let handler = scope.clone();
    host.mount(
        group().fill((
            button().label("Background").node_ref(background),
            view()
                .role(Role::Dialog)
                .name("Dialog")
                .focus(FocusMode::Programmatic)
                .style(Style {
                    background: Some(Color(255, 255, 255, 255)),
                    ..Style::new()
                        .width(Length::Px(220.))
                        .height(Length::Px(130.))
                })
                .semantics(Some(Box::new(Semantics {
                    id: Some(name("modal")),
                    layer: Some(Layer::default()),
                    ..Default::default()
                })))
                .fill((
                    button().label("Before"),
                    button()
                        .label("Trigger")
                        .node_ref(trigger)
                        .semantics(Some(Box::new(Semantics {
                            id: Some(name("trigger")),
                            ..Default::default()
                        }))),
                    button().label("After").node_ref(after),
                )),
            view()
                .role(Role::Menu)
                .name("Context actions")
                .focus(FocusMode::Programmatic)
                .node_ref(panel)
                .style(Style {
                    background: Some(Color(240, 240, 240, 255)),
                    ..Style::new()
                        .width(Length::Px(180.))
                        .height(Length::Px(100.))
                })
                .semantics(open.map(|open| {
                    let mut s = menu_metadata("panel", "trigger", !*open);
                    s.as_mut().unwrap().layer.as_mut().unwrap().placement =
                        LayerPlacement::ContextPoint(ContextPoint { x: 630., y: 470. });
                    s
                }))
                .on(EventKind::Menu, move |event| {
                    if let Event::Menu {
                        command: MenuCommand::Exit { backwards },
                        ..
                    } = event
                    {
                        open.revise(|v| *v = false);
                        handler
                            .action(Action::ExitMenu {
                                node: panel.get().unwrap(),
                                backwards,
                            })
                            .unwrap();
                    }
                })
                .on(EventKind::Dismiss, move |_| open.revise(|v| *v = false))
                .fill(
                    button()
                        .label("菜单 action")
                        .focus(FocusMode::TabStop)
                        .node_ref(item)
                        .semantics(member(MenuItemKind::Action))
                        .on(EventKind::Activate, move |_| picked.revise(|v| *v += 1)),
                ),
        )),
    )
    .unwrap();
    let (panel, item, trigger, after, background) = (
        panel.get().unwrap(),
        item.get().unwrap(),
        trigger.get().unwrap(),
        after.get().unwrap(),
        background.get().unwrap(),
    );
    ModalFixture {
        host,
        panel,
        item,
        trigger,
        after,
        background,
        open,
        picked,
    }
}
#[cfg(feature = "raster")]
#[test]
fn native_menu_context_popup_modal_scope_tab_exit_and_actual_cpu_frames_at_one_and_two_scales() {
    for scale in [1., 2.] {
        let mut f = modal_fixture(scale);
        settle(&mut f.host);
        assert_eq!(f.host.renderer.0.borrow().focused, Some(f.item));
        let physical = f.host.renderer.engine_id(f.panel, Part::Main).unwrap();
        let mut raster = crate::Raster::new();
        raster.draw(&f.host.renderer).unwrap();
        let open = raster.pixels().to_vec();
        f.host.renderer.inspect(|doc| {
            let r = doc.get_client_bounding_rect(physical).unwrap();
            assert!(
                (r.x - 452.).abs() < 0.1 && (r.y - 372.).abs() < 0.1,
                "{r:?}"
            );
        });
        if let Ok(path) = std::env::var("GLORY_MENU_ARTIFACTS") {
            std::fs::create_dir_all(&path).unwrap();
            image::save_buffer(
                format!("{path}/modal-context-{scale}.png"),
                raster.pixels(),
                (640. * scale) as u32,
                (480. * scale) as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
        key(&mut f.host, Key::Named(NamedKey::Tab), Modifiers::empty());
        assert!(!f.open.get());
        assert_eq!(f.host.renderer.0.borrow().focused, Some(f.after));
        assert!(
            f.host
                .app
                .scope()
                .action(Action::Focus(f.background))
                .is_ok()
        );
        f.host.app.flush().unwrap();
        settle(&mut f.host);
        assert_ne!(f.host.renderer.0.borrow().focused, Some(f.background));
        raster.draw(&f.host.renderer).unwrap();
        assert_ne!(raster.pixels(), open);
        f.open.revise(|v| *v = true);
        f.host.app.flush().unwrap();
        settle(&mut f.host);
        assert_eq!(
            f.host.renderer.engine_id(f.panel, Part::Main).unwrap(),
            physical
        );
        key(
            &mut f.host,
            Key::Named(NamedKey::Escape),
            Modifiers::empty(),
        );
        assert_eq!(f.host.renderer.0.borrow().focused, Some(f.trigger));
        assert_eq!(f.picked.get(), 0);
        f.host.app.clear().unwrap();
        assert!(f.host.renderer.0.borrow().dom.doc.top_layers().is_empty());
    }
}
#[test]
fn native_menu_exit_rejects_suspended_menu_without_focus_changes() {
    let mut host = WindowApp::new();
    let scope = host.app.scope();
    let trigger = scope
        .mount_with(|| {
            button()
                .label("Trigger")
                .semantics(Some(Box::new(Semantics {
                    id: Some(name("trigger")),
                    ..Default::default()
                })))
        })
        .unwrap();
    let mounted = scope
        .mount_with(|| {
            view()
                .role(Role::Menu)
                .name("Menu")
                .focus(FocusMode::Programmatic)
                .semantics(menu_metadata("panel", "trigger", true))
        })
        .unwrap();
    host.app.flush().unwrap();
    focus(&mut host, trigger.roots()[0]);
    mounted.suspend().unwrap();
    host.app.flush().unwrap();
    let before = host.renderer.0.borrow().focused;
    let result = host.renderer.clone().action(Action::ExitMenu {
        node: mounted.roots()[0],
        backwards: false,
    });
    assert!(matches!(
        result,
        Err(Error::Unsupported("menu exit on disconnected range"))
    ));
    assert_eq!(host.renderer.0.borrow().focused, before);
}

#[test]
fn native_menu_scrolled_panel_hit_uses_border_box_without_false_outside_dismissal() {
    use glory_render::style::{Length, Style};
    let mut host = WindowApp::new();
    host.renderer.set_viewport(400, 240, 1.).unwrap();
    let scope = host.app.scope();
    let panel = scope.cage(None);
    let selected = scope.cage(0usize);
    let dismiss = scope.cage(0usize);
    let last = scope.cage(None);
    let mut items: Vec<Box<dyn glory_ui::Widget>> = Vec::new();
    for i in 0..30 {
        let mut item = button()
            .label(format!("Item {i}"))
            .style(Style::new().height(Length::Px(30.)))
            .semantics(member(MenuItemKind::Action))
            .on(EventKind::Activate, move |_| selected.revise(|v| *v += 1));
        if i == 29 {
            item = item.node_ref(last);
        }
        items.push(Box::new(item));
    }
    host.mount(
        group().fill((
            button()
                .label("Trigger")
                .semantics(Some(Box::new(Semantics {
                    id: Some(name("trigger")),
                    ..Default::default()
                }))),
            view()
                .role(Role::Menu)
                .name("Scrollable menu")
                .focus(FocusMode::Programmatic)
                .node_ref(panel)
                .semantics(menu_metadata("panel", "trigger", false))
                .style(
                    Style::new()
                        .display(glory_render::style::Display::Flex)
                        .flex_direction(glory_render::style::FlexDirection::Column)
                        .width(Length::Px(180.))
                        .height(Length::Px(90.)),
                )
                .on(EventKind::Dismiss, move |_| dismiss.revise(|v| *v += 1))
                .fill(items),
        )),
    )
    .unwrap();
    let panel = panel.get().unwrap();
    host.app
        .scope()
        .action(Action::ScrollTo {
            node: panel,
            x: 0.,
            y: 10000.,
        })
        .unwrap();
    host.app.flush().unwrap();
    settle(&mut host);
    let rect = {
        let mut state = host.renderer.0.borrow_mut();
        state.dom.resolve();
        let panel = state.dom.main(panel).unwrap();
        assert!(state.dom.doc.get_node(panel).unwrap().scroll_offset().y > 0.);
        state
            .dom
            .doc
            .get_client_bounding_rect(state.dom.main(last.get().unwrap()).unwrap())
            .unwrap()
    };
    let (x, y) = ((rect.x + 10.) as f32, (rect.y + rect.height / 2.) as f32);
    host.handle_ui_event(UiEvent::PointerMove(pointer(x, y, false)));
    host.handle_ui_event(UiEvent::PointerDown(pointer(x, y, true)));
    host.handle_ui_event(UiEvent::PointerUp(pointer(x, y, false)));
    settle(&mut host);
    assert_eq!(dismiss.get(), 0);
    assert_eq!(selected.get(), 1);
}
