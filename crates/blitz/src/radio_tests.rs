use super::focus_tests::{focus, key, settle};
use super::*;
use glory_render::{Change, CheckMode, FieldPart, FormControl, Patch, SemanticId, Semantics};
use glory_ui::{button, check, events, group, view};

fn metadata(name: &str) -> Option<Box<Semantics>> {
    Some(Box::new(Semantics {
        check_group: Some(SemanticId::new(name).unwrap()),
        ..Default::default()
    }))
}
fn checked(app: &WindowApp, id: NodeId) -> bool {
    let control = app.renderer.engine_id(id, Part::Control).unwrap();
    app.renderer.inspect(|doc| {
        doc.get_node(control)
            .unwrap()
            .element_data()
            .unwrap()
            .checkbox_input_checked()
            .unwrap()
    })
}
fn click(app: &mut WindowApp, id: NodeId, part: Part) {
    let control = app.renderer.engine_id(id, part).unwrap();
    let event = app.renderer.inspect(|doc| {
        doc.get_node(control)
            .unwrap()
            .synthetic_click_event_data(Modifiers::empty())
    });
    let renderer = app.renderer.clone();
    EventDriver::new(app, Events::new(renderer))
        .handle_dom_event(DomEvent::new(control, DomEventData::Click(event)));
    settle(app);
}
fn space(app: &mut WindowApp) {
    key(app, Key::Character(" ".into()), Modifiers::empty());
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
    settle(app);
}

#[test]
fn radio_label_control_space_and_repeated_activation_publish_only_one_way_target_changes() {
    let mut app = WindowApp::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let selected = app.app.scope().cage(0usize);
    let make = |index: usize, initial: bool| {
        let seen = seen.clone();
        let value: glory_core::Lotus<bool> = if index < 2 {
            app.app.scope().bond(move || selected.get() == index).into()
        } else {
            initial.into()
        };
        check()
            .label(format!("Radio {index}"))
            .mode(CheckMode::Radio)
            .checked(value)
            .semantics(metadata("choice"))
            .on(events::checked_changed, move |event| {
                if index < 2 && event == Event::CheckedChanged(true) {
                    selected.revise(|value| *value = index);
                }
                seen.borrow_mut().push((index, event))
            })
    };
    app.mount(
        group().fill((
            view()
                .semantics(Some(Box::new(Semantics {
                    field_part: Some(FieldPart::Form),
                    ..Default::default()
                })))
                .fill((
                    make(0, true),
                    make(1, false),
                    check().checked(true).semantics(Some(Box::new(Semantics {
                        form_control: Some(FormControl {
                            name: "choice".into(),
                            value: Some("checkbox".into()),
                        }),
                        ..Default::default()
                    }))),
                )),
            view()
                .semantics(Some(Box::new(Semantics {
                    field_part: Some(FieldPart::Form),
                    ..Default::default()
                })))
                .fill(make(2, true)),
        )),
    )
    .unwrap();
    settle(&mut app);
    let root = app.app.tree().roots()[0];
    let forms = app.app.tree().node(root).unwrap().children.clone();
    let first = app.app.tree().node(forms[0]).unwrap().children.clone();
    let c = app.app.tree().node(forms[1]).unwrap().children[0];
    let (a, b, checkbox) = (first[0], first[1], first[2]);
    assert!(checked(&app, a) && checked(&app, c) && checked(&app, checkbox));
    click(&mut app, b, Part::Main);
    assert_eq!(
        selected.get(),
        1,
        "the accepted proposal updates the controlled source in its host tick"
    );
    assert!(!checked(&app, a) && checked(&app, b) && checked(&app, c) && checked(&app, checkbox));
    click(&mut app, b, Part::Control);
    focus(&mut app, b);
    space(&mut app);
    assert_eq!(*seen.borrow(), vec![(1, Event::CheckedChanged(true))]);
    focus(&mut app, a);
    space(&mut app);
    assert_eq!(selected.get(), 0);
    assert!(checked(&app, a) && !checked(&app, b) && checked(&app, c) && checked(&app, checkbox));
    assert_eq!(
        *seen.borrow(),
        vec![
            (1, Event::CheckedChanged(true)),
            (0, Event::CheckedChanged(true))
        ]
    );
}

#[test]
fn radio_program_reassertion_repairs_rejected_proposals_and_dynamic_association_keeps_identity() {
    let mut app = WindowApp::new();
    app.mount(
        group().fill((
            check()
                .label("A")
                .mode(CheckMode::Radio)
                .checked(true)
                .semantics(metadata("choice")),
            check()
                .label("B")
                .mode(CheckMode::Radio)
                .semantics(metadata("choice")),
        )),
    )
    .unwrap();
    settle(&mut app);
    let root = app.app.tree().roots()[0];
    let nodes = app.app.tree().node(root).unwrap().children.clone();
    let a = nodes[0];
    let b = nodes[1];
    let physical = app.renderer.engine_id(b, Part::Control).unwrap();
    click(&mut app, b, Part::Control);
    assert!(!checked(&app, a) && checked(&app, b));
    // The accepted semantic value of A is still true. Equal-value property
    // reassertion must repair the peer change made by the native proposal.
    app.app
        .scope()
        .patch(a, Patch::Checked(Change::Set(true)))
        .unwrap();
    settle(&mut app);
    assert!(checked(&app, a) && !checked(&app, b));
    app.app
        .scope()
        .patch(b, Patch::Semantics(Change::Set(metadata("other"))))
        .unwrap();
    app.app
        .scope()
        .patch(b, Patch::Checked(Change::Set(true)))
        .unwrap();
    settle(&mut app);
    assert!(checked(&app, a) && checked(&app, b));
    app.app
        .scope()
        .patch(b, Patch::Semantics(Change::Set(metadata("choice"))))
        .unwrap();
    settle(&mut app);
    assert!(!checked(&app, a) && checked(&app, b));
    app.app
        .scope()
        .patch(b, Patch::Semantics(Change::Clear))
        .unwrap();
    app.app
        .scope()
        .patch(b, Patch::CheckMode(Change::Set(CheckMode::Checkbox)))
        .unwrap();
    settle(&mut app);
    click(&mut app, b, Part::Control);
    assert!(!checked(&app, b));
    assert_eq!(app.renderer.engine_id(b, Part::Control).unwrap(), physical);
}

#[test]
fn intrinsic_radio_arrows_wrap_skip_disabled_and_readonly_members_and_tab_has_one_stop() {
    let mut app = WindowApp::new();
    app.mount(
        group().fill((
            button().label("before"),
            check()
                .label("A")
                .mode(CheckMode::Radio)
                .checked(true)
                .semantics(metadata("choice")),
            check()
                .label("disabled")
                .mode(CheckMode::Radio)
                .enabled(false)
                .semantics(metadata("choice")),
            check()
                .label("B")
                .mode(CheckMode::Radio)
                .semantics(metadata("choice")),
            button().label("after"),
        )),
    )
    .unwrap();
    settle(&mut app);
    let root = app.app.tree().roots()[0];
    let ids = app.app.tree().node(root).unwrap().children.clone();
    let focused = |app: &WindowApp, id| {
        let state = app.renderer.0.borrow();
        state.dom.doc.get_focussed_node_id() == state.dom.control(id).ok()
    };
    focus(&mut app, ids[0]);
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert!(focused(&app, ids[1]));
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert!(focused(&app, ids[4]));
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::SHIFT);
    assert!(focused(&app, ids[1]));
    key(
        &mut app,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
    );
    assert!(focused(&app, ids[3]));
    assert!(checked(&app, ids[3]) && !checked(&app, ids[1]));
    key(
        &mut app,
        Key::Named(NamedKey::ArrowRight),
        Modifiers::empty(),
    );
    assert!(focused(&app, ids[1]));
    key(&mut app, Key::Named(NamedKey::End), Modifiers::empty());
    assert!(focused(&app, ids[3]));
    key(&mut app, Key::Named(NamedKey::Home), Modifiers::empty());
    assert!(focused(&app, ids[1]));
    let mut readonly = *metadata("choice").unwrap();
    readonly.readonly = true;
    app.app
        .scope()
        .patch(
            ids[3],
            Patch::Semantics(Change::Set(Some(Box::new(readonly)))),
        )
        .unwrap();
    settle(&mut app);
    key(
        &mut app,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
    );
    assert!(focused(&app, ids[1]));
    click(&mut app, ids[3], Part::Control);
    assert!(!checked(&app, ids[3]));
    focus(&mut app, ids[3]);
    key(&mut app, Key::Named(NamedKey::ArrowUp), Modifiers::empty());
    assert!(
        focused(&app, ids[1]),
        "a readonly member must not trap navigation to an editable peer"
    );
}

#[test]
fn explicit_radio_navigation_stays_with_the_component_and_preserves_its_roving_tab_policy() {
    use glory_render::{FocusMode, Navigation, Orientation, Role};
    let mut app = WindowApp::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let output = seen.clone();
    let mut member = *metadata("explicit").unwrap();
    member.navigation_item = true;
    app.mount(
        group().fill((
            button().label("before"),
            view()
                .role(Role::RadioGroup)
                .semantics(Some(Box::new(Semantics {
                    navigation: Some(Orientation::Horizontal),
                    ..Default::default()
                })))
                .fill((
                    check()
                        .label("A")
                        .mode(CheckMode::Radio)
                        .checked(true)
                        .focus(FocusMode::TabStop)
                        .semantics(Some(Box::new(member.clone()))),
                    check()
                        .label("B")
                        .mode(CheckMode::Radio)
                        .focus(FocusMode::Programmatic)
                        .semantics(Some(Box::new(member))),
                ))
                .on(events::navigate, move |event| {
                    output.borrow_mut().push(event)
                }),
            button().label("after"),
        )),
    )
    .unwrap();
    settle(&mut app);
    let root = app.app.tree().roots()[0];
    let outer = app.app.tree().node(root).unwrap().children.clone();
    let members = app.app.tree().node(outer[1]).unwrap().children.clone();
    focus(&mut app, members[0]);
    key(
        &mut app,
        Key::Named(NamedKey::ArrowRight),
        Modifiers::empty(),
    );
    assert_eq!(
        *seen.borrow(),
        vec![Event::Navigate {
            from: members[0],
            command: Navigation::Next
        }]
    );
    assert!(
        checked(&app, members[0]) && !checked(&app, members[1]),
        "the component decides whether to accept navigation"
    );
    key(
        &mut app,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
    );
    assert_eq!(seen.borrow().len(), 1);
    assert!(
        checked(&app, members[0]) && !checked(&app, members[1]),
        "keys outside the configured axis cannot invoke an intrinsic selection policy"
    );
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(
        app.renderer.0.borrow().dom.doc.get_focussed_node_id(),
        app.renderer.0.borrow().dom.control(outer[2]).ok()
    );
}

#[test]
fn suspended_radio_has_an_independent_tree_rejoins_once_and_retired_engine_ids_cannot_select() {
    let mut app = WindowApp::new();
    let a = check()
        .label("A")
        .mode(CheckMode::Radio)
        .checked(true)
        .semantics(metadata("lifecycle"))
        .mount(app.app.scope())
        .unwrap();
    let _b = check()
        .label("B")
        .mode(CheckMode::Radio)
        .semantics(metadata("lifecycle"))
        .mount(app.app.scope())
        .unwrap();
    settle(&mut app);
    let roots = app.app.tree().roots().to_vec();
    let (first, second) = (roots[0], roots[1]);
    let old = app.renderer.engine_id(first, Part::Control).unwrap();
    a.suspend().unwrap();
    settle(&mut app);
    app.app
        .scope()
        .patch(second, Patch::Checked(Change::Set(true)))
        .unwrap();
    settle(&mut app);
    assert!(checked(&app, first) && checked(&app, second));
    a.place_before(None).unwrap();
    settle(&mut app);
    assert!(checked(&app, first) && !checked(&app, second));
    assert_eq!(app.renderer.engine_id(first, Part::Control).unwrap(), old);
    drop(a);
    settle(&mut app);
    let _replacement = check()
        .label("New")
        .mode(CheckMode::Radio)
        .checked(true)
        .semantics(metadata("lifecycle"))
        .mount(app.app.scope())
        .unwrap();
    settle(&mut app);
    let mut state = app.renderer.0.borrow_mut();
    assert_eq!(state.dom.doc.activate_radio(old), None);
    assert!(state.dom.doc.radio_group(old).is_empty());
}

#[cfg(feature = "raster")]
#[test]
fn cpu_radio_pixels_repaint_both_peers_and_restore_the_initial_frame() {
    let app = WindowApp::new();
    app.renderer.set_viewport(240, 160, 1.).unwrap();
    app.mount(
        glory_ui::col()
            .style(glory_render::style::Style {
                display: Some(glory_render::style::Display::Flex),
                flex_direction: Some(glory_render::style::FlexDirection::Column),
                ..Default::default()
            })
            .fill((
                check()
                    .label("A")
                    .mode(CheckMode::Radio)
                    .checked(true)
                    .semantics(metadata("paint")),
                check()
                    .label("B")
                    .mode(CheckMode::Radio)
                    .semantics(metadata("paint")),
            )),
    )
    .unwrap();
    app.app.settle(16).unwrap();
    let root = app.app.tree().roots()[0];
    let ids = app.app.tree().node(root).unwrap().children.clone();
    let mut raster = Raster::new();
    raster.draw(&app.renderer).unwrap();
    let initial = raster.pixels().to_vec();
    let crop = |pixels: &[u8], id| {
        let control = app.renderer.engine_id(id, Part::Control).unwrap();
        let rect = app
            .renderer
            .inspect(|doc| doc.get_client_bounding_rect(control).unwrap());
        let mut region = Vec::new();
        for y in rect.y.floor() as usize..(rect.y + rect.height).ceil() as usize {
            for x in rect.x.floor() as usize..(rect.x + rect.width).ceil() as usize {
                region.extend_from_slice(&pixels[(y * 240 + x) * 4..(y * 240 + x + 1) * 4]);
            }
        }
        region
    };
    app.app
        .scope()
        .patch(ids[1], Patch::Checked(Change::Set(true)))
        .unwrap();
    app.app.flush().unwrap();
    raster.draw(&app.renderer).unwrap();
    assert!(crop(&initial, ids[0]) != crop(raster.pixels(), ids[0]));
    assert!(crop(&initial, ids[1]) != crop(raster.pixels(), ids[1]));
    app.app
        .scope()
        .patch(ids[0], Patch::Checked(Change::Set(true)))
        .unwrap();
    app.app.flush().unwrap();
    raster.draw(&app.renderer).unwrap();
    assert!(
        initial == raster.pixels(),
        "programmatic replacement restores both circle states without stale pixels"
    );
}
