use super::focus_tests::{focus, key, selection, settle};
use super::tests::{bounds_now, pointer};
use super::*;
use glory_core::Cage;
use glory_render::{
    ChoiceCommand, ComboboxKind, FocusMode, Layer, LayerFocus, LayerPlacement, OptionSemantics,
    Orientation, PopupKind, Role, SemanticId, Semantics,
};
use glory_ui::{Widget, button, group, input, view};

pub(super) const CSS: &str = "body{margin:0;background:white}.trigger{width:180px;height:32px}.panel{width:180px;height:112px;overflow-y:auto;display:flex;flex-direction:column;background:white}.option{height:28px;min-height:28px;width:160px;margin:0;padding:0}";
pub(super) fn name(s: &str) -> SemanticId {
    SemanticId::new(s).unwrap()
}
pub(super) struct Fixture {
    pub host: WindowApp,
    pub trigger: NodeId,
    pub panel: NodeId,
    pub options: Vec<NodeId>,
    pub open: Cage<bool>,
    pub current: Cage<usize>,
    pub selected: Cage<Option<usize>>,
    pub draft: Cage<String>,
    pub readonly: Cage<bool>,
    pub enabled: Cage<bool>,
    pub commands: Rc<RefCell<Vec<ChoiceCommand>>>,
    pub searches: Rc<RefCell<Vec<(String, u64)>>>,
    pub picked: Rc<Cell<usize>>,
}
pub(super) fn fixture(kind: ComboboxKind) -> Fixture {
    let host = WindowApp::new();
    host.renderer.set_viewport(400, 300, 1.).unwrap();
    host.renderer.set_css(CSS);
    let scope = host.app.scope();
    let open = scope.cage(false);
    let current = scope.cage(0usize);
    let selected = scope.cage(None::<usize>);
    let draft = scope.cage(String::new());
    let readonly = scope.cage(false);
    let enabled = scope.cage(true);
    let commands = Rc::new(RefCell::new(Vec::new()));
    let received = commands.clone();
    let searches = Rc::new(RefCell::new(Vec::new()));
    let searched = searches.clone();
    let picked = Rc::new(Cell::new(0));
    let committed = picked.clone();
    let activated = picked.clone();
    let metadata = scope.bond(move || {
        Some(Box::new(Semantics {
            id: Some(name("choice-trigger")),
            combobox: Some(kind),
            controls: Some(name("choice-panel")),
            popup: Some(PopupKind::Listbox),
            expanded: Some(open.get()),
            readonly: readonly.get(),
            typeahead: kind == ComboboxKind::Select,
            active_descendant: open
                .get()
                .then(|| name(&format!("option-{}", current.get()))),
            ..Default::default()
        }))
    });
    let command = move |event| {
        let Event::Choice { command, .. } = event else {
            return;
        };
        received.borrow_mut().push(command);
        match command {
            ChoiceCommand::OpenFirst => {
                current.revise(|v| *v = 0);
                open.revise(|v| *v = true);
            }
            ChoiceCommand::OpenLast => {
                current.revise(|v| *v = 2);
                open.revise(|v| *v = true);
            }
            ChoiceCommand::Next | ChoiceCommand::Previous => {
                if open.get() {
                    current.revise(|v| *v = if *v == 0 { 2 } else { 0 });
                } else {
                    current.revise(|v| *v = 0);
                    open.revise(|v| *v = true);
                }
            }
            ChoiceCommand::First => current.revise(|v| *v = 0),
            ChoiceCommand::Last => current.revise(|v| *v = 2),
            ChoiceCommand::Commit => {
                if open.get() {
                    selected.revise(|v| *v = Some(current.get()));
                    committed.set(committed.get() + 1);
                    open.revise(|v| *v = false);
                } else {
                    open.revise(|v| *v = true);
                }
            }
            ChoiceCommand::Close => open.revise(|v| *v = false),
            ChoiceCommand::RemoveLast => selected.revise(|v| *v = None),
        }
    };
    let trigger_ref = scope.cage(None);
    let panel_ref = scope.cage(None);
    let trigger: Box<dyn Widget> = if kind == ComboboxKind::Select {
        Box::new(
            button()
                .label("Choose")
                .enabled(enabled)
                .class("trigger")
                .name("Choice")
                .node_ref(trigger_ref)
                .semantics(metadata)
                .on(EventKind::Choice, command)
                .on(EventKind::Typeahead, move |event| {
                    if let Event::Typeahead {
                        text, at_millis, ..
                    } = event
                    {
                        searched.borrow_mut().push((text, at_millis));
                    }
                })
                .on(EventKind::Activate, move |_| {
                    activated.set(activated.get() + 1);
                    open.revise(|v| *v = !*v);
                }),
        )
    } else {
        Box::new(
            input()
                .value(draft)
                .enabled(enabled)
                .class("trigger")
                .name("Choice")
                .node_ref(trigger_ref)
                .semantics(metadata)
                .on(EventKind::Choice, command)
                .on(EventKind::ValueChanged, move |event| {
                    if let Event::ValueChanged { value, .. } = event {
                        draft.revise(|v| *v = value);
                    }
                }),
        )
    };
    let mut options: Vec<Box<dyn Widget>> = Vec::new();
    let mut refs = Vec::new();
    for i in 0..3 {
        let reference = scope.cage(None);
        refs.push(reference);
        let count = picked.clone();
        options.push(Box::new(
            button()
                .label(format!("Option {i}"))
                .enabled(i != 1)
                .focus(FocusMode::Programmatic)
                .node_ref(reference)
                .class("option")
                .semantics(selected.map(move |selected| {
                    Some(Box::new(Semantics {
                        id: Some(name(&format!("option-{i}"))),
                        option: Some(OptionSemantics {
                            owner: name("choice-panel"),
                        }),
                        selected: Some(*selected == Some(i)),
                        ..Default::default()
                    }))
                }))
                .on(EventKind::Activate, move |_| {
                    count.set(count.get() + 1);
                    selected.revise(|v| *v = Some(i));
                    open.revise(|v| *v = false);
                })
                .on(EventKind::Pointer, move |event| {
                    if i != 1
                        && matches!(
                            event,
                            Event::Pointer {
                                phase: glory_render::PointerPhase::Move,
                                ..
                            }
                        )
                    {
                        current.revise(|v| *v = i);
                    }
                }),
        ));
    }
    host.mount(
        group().fill((
            trigger,
            view()
                .role(Role::Listbox)
                .class("panel")
                .node_ref(panel_ref)
                .semantics(open.map(move |open| {
                    Some(Box::new(Semantics {
                        id: Some(name("choice-panel")),
                        navigation: Some(Orientation::Vertical),
                        listbox_multiple: kind == ComboboxKind::Multiple,
                        hidden: !*open,
                        layer: Some(Layer {
                            modal: false,
                            focus: LayerFocus::Preserve,
                            placement: LayerPlacement::Anchor(glory_render::AnchorPlacement::new(
                                glory_render::AnchorSide::Bottom,
                                glory_render::AnchorAlign::Start,
                            )),
                            anchor: Some(name("choice-trigger")),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .on(EventKind::Dismiss, move |_| open.revise(|v| *v = false))
                .fill(options),
            button().label("After"),
        )),
    )
    .unwrap();
    let trigger = trigger_ref.get().unwrap();
    let panel = panel_ref.get().unwrap();
    let options = refs.iter().map(|r| r.get().unwrap()).collect();
    Fixture {
        host,
        trigger,
        panel,
        options,
        open,
        current,
        selected,
        draft,
        readonly,
        enabled,
        commands,
        searches,
        picked,
    }
}
fn focused(host: &WindowApp) -> Option<NodeId> {
    let state = host.renderer.0.borrow();
    state
        .dom
        .doc
        .get_focussed_node_id()
        .and_then(|id| state.dom.owner(id))
}

#[test]
fn native_choice_routes_bursts_against_component_state_and_preserves_popup_focus_on_hover_click_tab_and_escape()
 {
    let mut f = fixture(ComboboxKind::Select);
    focus(&mut f.host, f.trigger);
    let native = |key| {
        UiEvent::KeyDown(BlitzKeyEvent {
            key,
            code: keyboard_types::Code::Unidentified,
            modifiers: Modifiers::empty(),
            location: keyboard_types::Location::Standard,
            is_auto_repeating: false,
            is_composing: false,
            state: blitz_traits::events::KeyState::Pressed,
            text: None,
        })
    };
    f.host
        .handle_ui_event(native(Key::Named(NamedKey::ArrowDown)));
    f.host
        .handle_ui_event(native(Key::Named(NamedKey::ArrowDown)));
    settle(&mut f.host);
    assert_eq!(
        *f.commands.borrow(),
        [ChoiceCommand::Next, ChoiceCommand::Next]
    );
    assert!(f.open.get());
    assert_eq!(f.current.get(), 2);
    assert_eq!(focused(&f.host), Some(f.trigger));
    let rect = bounds_now(&f.host, f.options[0]);
    f.host.handle_ui_event(UiEvent::PointerMove(pointer(
        (rect.x + 10.) as f32,
        (rect.y + 10.) as f32,
        false,
    )));
    settle(&mut f.host);
    assert_eq!(f.current.get(), 0);
    f.host.handle_ui_event(UiEvent::PointerDown(pointer(
        (rect.x + 10.) as f32,
        (rect.y + 10.) as f32,
        true,
    )));
    settle(&mut f.host);
    assert_eq!(focused(&f.host), Some(f.trigger));
    f.host.handle_ui_event(UiEvent::PointerUp(pointer(
        (rect.x + 10.) as f32,
        (rect.y + 10.) as f32,
        false,
    )));
    settle(&mut f.host);
    assert_eq!(f.selected.get(), Some(0));
    assert_eq!(f.picked.get(), 1);
    assert!(!f.open.get());
    assert_eq!(focused(&f.host), Some(f.trigger));
    key(
        &mut f.host,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
    );
    key(
        &mut f.host,
        Key::Named(NamedKey::Escape),
        Modifiers::empty(),
    );
    assert!(!f.open.get());
    assert_eq!(f.commands.borrow().last(), Some(&ChoiceCommand::Close));
    key(&mut f.host, Key::Character("中".into()), Modifiers::empty());
    assert_eq!(f.searches.borrow()[0].0, "中");
    key(
        &mut f.host,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
    );
    let root = f.host.app.tree().roots()[0];
    let after = f.host.app.tree().node(root).unwrap().children[2];
    key(&mut f.host, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(
        focused(&f.host),
        Some(after),
        "options are absent from the Tab sequence"
    );
    focus(&mut f.host, f.trigger);
    key(
        &mut f.host,
        Key::Named(NamedKey::Escape),
        Modifiers::empty(),
    );
    let before = f.commands.borrow().len();
    f.readonly.revise(|v| *v = true);
    f.host.app.flush().unwrap();
    key(
        &mut f.host,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
    );
    assert_eq!(f.commands.borrow().len(), before);
    f.readonly.revise(|v| *v = false);
    f.enabled.revise(|v| *v = false);
    f.host.app.flush().unwrap();
    key(&mut f.host, Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(f.picked.get(), 1);
}

fn backspace_pair(host: &mut WindowApp, modifiers: Modifiers) {
    #[cfg(target_os = "macos")]
    host.handle_ui_event(UiEvent::AppleStandardKeybinding("deleteBackward:".into()));
    host.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
        key: Key::Named(NamedKey::Backspace),
        code: keyboard_types::Code::Unidentified,
        modifiers,
        location: keyboard_types::Location::Standard,
        is_auto_repeating: false,
        is_composing: false,
        state: blitz_traits::events::KeyState::Pressed,
        text: None,
    }));
    settle(host);
}

#[test]
fn native_editable_choice_keeps_caret_text_ime_and_multiple_backspace_independent_of_choice_commands()
 {
    for kind in [
        ComboboxKind::Editable,
        ComboboxKind::Autocomplete,
        ComboboxKind::Multiple,
    ] {
        let mut f = fixture(kind);
        focus(&mut f.host, f.trigger);
        let physical = f.host.renderer.engine_id(f.trigger, Part::Main).unwrap();
        key(
            &mut f.host,
            Key::Named(NamedKey::ArrowDown),
            Modifiers::empty(),
        );
        assert_eq!(f.commands.borrow().last(), Some(&ChoiceCommand::OpenFirst));
        assert!(f.open.get());
        key(&mut f.host, Key::Character("A".into()), Modifiers::empty());
        assert_eq!(f.draft.get(), "A");
        let before = selection(&mut f.host, f.trigger);
        let count = f.commands.borrow().len();
        f.host.handle_ui_event(UiEvent::Ime(BlitzImeEvent::Preedit(
            "拼".into(),
            Some((3, 3)),
        )));
        settle(&mut f.host);
        key(
            &mut f.host,
            Key::Named(NamedKey::ArrowDown),
            Modifiers::empty(),
        );
        assert_eq!(f.commands.borrow().len(), count);
        f.current.revise(|v| *v = 2);
        f.host.app.flush().unwrap();
        settle(&mut f.host);
        f.host.renderer.inspect(|doc| {
            assert!(
                doc.get_node(physical)
                    .unwrap()
                    .element_data()
                    .unwrap()
                    .text_input_data()
                    .unwrap()
                    .editor
                    .is_composing()
            )
        });
        f.host
            .handle_ui_event(UiEvent::Ime(BlitzImeEvent::Preedit("".into(), None)));
        settle(&mut f.host);
        assert_eq!(selection(&mut f.host, f.trigger), before);
        assert_eq!(f.draft.get(), "A");
        key(
            &mut f.host,
            Key::Named(NamedKey::ArrowLeft),
            Modifiers::empty(),
        );
        assert_eq!(selection(&mut f.host, f.trigger), (0, 0));
        key(&mut f.host, Key::Named(NamedKey::End), Modifiers::empty());
        backspace_pair(&mut f.host, Modifiers::empty());
        assert!(f.draft.get().is_empty());
        assert_eq!(f.commands.borrow().len(), count);
        backspace_pair(&mut f.host, Modifiers::empty());
        assert_eq!(
            f.commands.borrow().len(),
            count + usize::from(kind == ComboboxKind::Multiple)
        );
        if kind == ComboboxKind::Multiple {
            assert_eq!(f.commands.borrow().last(), Some(&ChoiceCommand::RemoveLast));
        }
        assert_eq!(
            f.host.renderer.engine_id(f.trigger, Part::Main).unwrap(),
            physical
        );
    }
}

#[test]
fn option_reveal_scrolls_only_its_current_listbox_and_keeps_focus_and_viewport_fixed() {
    let mut f = fixture(ComboboxKind::Editable);
    f.host
        .renderer
        .set_css(&format!("{CSS}.panel{{height:40px}}"));
    focus(&mut f.host, f.trigger);
    key(
        &mut f.host,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
    );
    let panel = f.host.renderer.engine_id(f.panel, Part::Main).unwrap();
    let before = f.host.renderer.inspect(|doc| doc.viewport_scroll());
    f.host
        .app
        .scope()
        .action(glory_render::Action::RevealOption { node: f.options[2] })
        .unwrap();
    f.host.app.flush().unwrap();
    settle(&mut f.host);
    assert!(
        f.host
            .renderer
            .inspect(|doc| doc.get_node(panel).unwrap().scroll_offset().y)
            > 0.
    );
    assert_eq!(f.host.renderer.inspect(|doc| doc.viewport_scroll()), before);
    assert_eq!(focused(&f.host), Some(f.trigger));
    let old = f.options[2];
    f.host.app.clear().unwrap();
    assert!(
        f.host
            .app
            .scope()
            .action(glory_render::Action::RevealOption { node: old })
            .is_err()
    );
    assert!(f.host.renderer.0.borrow().layers.open.is_empty());
}

#[test]
fn collection_typeahead_uses_one_unicode_scalar_and_current_nearest_opted_in_boundary() {
    let mut host = WindowApp::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let received = seen.clone();
    host.mount(
        view()
            .semantics(Some(Box::new(Semantics {
                navigation: Some(Orientation::Vertical),
                typeahead: true,
                ..Default::default()
            })))
            .on(EventKind::Typeahead, move |event| {
                if let Event::Typeahead {
                    from,
                    text,
                    at_millis,
                } = event
                {
                    received.borrow_mut().push((from, text, at_millis));
                }
            })
            .fill((
                button().label("Item").semantics(Some(Box::new(Semantics {
                    navigation_item: true,
                    ..Default::default()
                }))),
                input(),
                button().label("Ordinary"),
                view()
                    .semantics(Some(Box::new(Semantics {
                        navigation: Some(Orientation::Vertical),
                        typeahead: true,
                        ..Default::default()
                    })))
                    .fill(button().label("Nested").semantics(Some(Box::new(Semantics {
                        navigation_item: true,
                        ..Default::default()
                    })))),
            )),
    )
    .unwrap();
    let root = host.app.tree().roots()[0];
    let children = host.app.tree().node(root).unwrap().children.clone();
    focus(&mut host, children[0]);
    for text in ["A", "中", "😀"] {
        key(&mut host, Key::Character(text.into()), Modifiers::empty());
    }
    assert_eq!(
        seen.borrow()
            .iter()
            .map(|e| e.1.as_str())
            .collect::<Vec<_>>(),
        ["A", "中", "😀"]
    );
    assert!(
        seen.borrow()
            .iter()
            .all(|e| e.0 == children[0] && e.2 <= glory_render::MAX_TYPEAHEAD_MILLIS)
    );
    assert!(seen.borrow().windows(2).all(|w| w[0].2 <= w[1].2));
    for (text, mods) in [
        (" ", Modifiers::empty()),
        ("ab", Modifiers::empty()),
        ("x", Modifiers::CONTROL),
        ("x", Modifiers::META),
        ("x", Modifiers::ALT),
    ] {
        key(&mut host, Key::Character(text.into()), mods);
    }
    assert_eq!(seen.borrow().len(), 3);
    focus(&mut host, children[1]);
    key(&mut host, Key::Character("x".into()), Modifiers::empty());
    assert_eq!(seen.borrow().len(), 3);
    focus(&mut host, children[2]);
    key(&mut host, Key::Character("x".into()), Modifiers::empty());
    assert_eq!(seen.borrow().len(), 3);
    let nested = host.app.tree().node(children[3]).unwrap().children[0];
    focus(&mut host, nested);
    key(&mut host, Key::Character("x".into()), Modifiers::empty());
    assert_eq!(
        seen.borrow().len(),
        3,
        "a nearer boundary without an interest must not search its ancestor"
    );
}

#[test]
fn mouse_pen_boundaries_keep_related_origins_across_logical_groups_and_touch_stays_out_of_hover() {
    for pointer_id in [
        blitz_traits::events::BlitzPointerId::Mouse,
        blitz_traits::events::BlitzPointerId::Pen,
    ] {
        use glory_render::PointerPhase;
        let mut host = WindowApp::new();
        host.renderer
            .set_css("body{margin:0}button{display:block;width:80px;height:30px}");
        let within = Rc::new(RefCell::new(Vec::new()));
        let changes = within.clone();
        let direct = Rc::new(RefCell::new(Vec::new()));
        let observed = direct.clone();
        host.mount(
            group().fill((
                view()
                    .on(EventKind::PointerWithin, move |event| {
                        changes.borrow_mut().push(event)
                    })
                    .fill(group().fill((
                        button().label("A").on(EventKind::Pointer, move |event| {
                            observed.borrow_mut().push(event)
                        }),
                        button().label("B"),
                    ))),
                button().label("outside"),
            )),
        )
        .unwrap();
        let group = host.app.tree().roots()[0];
        let children = host.app.tree().node(group).unwrap().children.clone();
        let inner = host.app.tree().node(children[0]).unwrap().children[0];
        let buttons = host.app.tree().node(inner).unwrap().children.clone();
        for id in [buttons[0], buttons[1], children[1]] {
            let b = bounds_now(&host, id);
            let mut moving = pointer((b.x + 5.) as f32, (b.y + 5.) as f32, false);
            moving.id = pointer_id;
            host.handle_ui_event(UiEvent::PointerMove(moving));
            settle(&mut host);
        }
        let boundaries = within
            .borrow()
            .iter()
            .filter_map(|e| {
                if let Event::PointerWithin {
                    from,
                    related,
                    phase,
                } = e
                {
                    (*phase != PointerPhase::Move).then_some((*from, *related, *phase))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            boundaries,
            [
                (buttons[0], None, PointerPhase::Enter),
                (buttons[1], Some(children[1]), PointerPhase::Leave)
            ]
        );
        assert_eq!(
            direct
                .borrow()
                .iter()
                .filter(|e| matches!(
                    e,
                    Event::Pointer {
                        phase: PointerPhase::Move,
                        ..
                    }
                ))
                .count(),
            1
        );
        let before = within.borrow().len();
        let b = bounds_now(&host, buttons[0]);
        let mut finger = pointer((b.x + 5.) as f32, (b.y + 5.) as f32, false);
        finger.id = blitz_traits::events::BlitzPointerId::Finger(1);
        host.handle_ui_event(UiEvent::PointerMove(finger));
        settle(&mut host);
        assert_eq!(within.borrow().len(), before);
    }
}

#[test]
fn tree_popup_moves_real_focus_to_tree_items_and_escape_restores_its_combobox_trigger() {
    use glory_render::{TreeItem, TreeNavigation};
    let mut host = WindowApp::new();
    host.renderer.set_css(CSS);
    let scope = host.app.scope();
    let open = scope.cage(false);
    let trigger = scope.cage(None);
    let item = scope.cage(None);
    host.mount(
        group().fill((
            button()
                .label("Choose tree item")
                .class("trigger")
                .node_ref(trigger)
                .semantics(open.map(move |open| {
                    Some(Box::new(Semantics {
                        id: Some(name("tree-trigger")),
                        combobox: Some(ComboboxKind::Select),
                        controls: Some(name("tree-popup")),
                        popup: Some(PopupKind::Tree),
                        expanded: Some(*open),
                        ..Default::default()
                    }))
                }))
                .on(EventKind::Choice, move |event| {
                    if matches!(
                        event,
                        Event::Choice {
                            command: ChoiceCommand::Next,
                            ..
                        }
                    ) {
                        open.revise(|v| *v = true);
                    }
                }),
            view()
                .role(Role::Tree)
                .focus(FocusMode::Programmatic)
                .name("Tree choices")
                .class("panel")
                .semantics(open.map(move |open| {
                    Some(Box::new(Semantics {
                        id: Some(name("tree-popup")),
                        hidden: !*open,
                        tree_navigation: Some(TreeNavigation::default()),
                        layer: Some(Layer {
                            modal: false,
                            placement: LayerPlacement::Anchor(glory_render::AnchorPlacement::new(
                                glory_render::AnchorSide::Bottom,
                                glory_render::AnchorAlign::Start,
                            )),
                            anchor: Some(name("tree-trigger")),
                            return_focus: Some(name("tree-trigger")),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .on(EventKind::Dismiss, move |_| open.revise(|v| *v = false))
                .fill(
                    view()
                        .role(Role::TreeItem)
                        .name("Leaf")
                        .focus(FocusMode::TabStop)
                        .node_ref(item)
                        .semantics(Some(Box::new(Semantics {
                            tree_item: Some(TreeItem {
                                level: 1,
                                position: 0,
                                count: 1,
                            }),
                            selected: Some(false),
                            ..Default::default()
                        }))),
                ),
        )),
    )
    .unwrap();
    let trigger = trigger.get().unwrap();
    focus(&mut host, trigger);
    key(
        &mut host,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
    );
    assert!(open.get());
    assert_eq!(focused(&host), item.get());
    key(&mut host, Key::Named(NamedKey::Escape), Modifiers::empty());
    assert!(!open.get());
    assert_eq!(focused(&host), Some(trigger));
    host.app.clear().unwrap();
    assert!(host.renderer.0.borrow().layers.open.is_empty());
}

#[cfg(feature = "raster")]
#[test]
fn native_choice_popup_has_real_cpu_pixels_and_closes_to_the_same_frame_without_replacing_the_editor()
 {
    use crate::Raster;
    for kind in [
        ComboboxKind::Select,
        ComboboxKind::Editable,
        ComboboxKind::Autocomplete,
        ComboboxKind::Multiple,
    ] {
        let f = fixture(kind);
        let physical = f.host.renderer.engine_id(f.trigger, Part::Main).unwrap();
        let mut raster = Raster::new();
        raster.draw(&f.host.renderer).unwrap();
        let initial = raster.pixels().to_vec();
        f.open.revise(|v| *v = true);
        f.host.app.settle(12).unwrap();
        raster.draw(&f.host.renderer).unwrap();
        assert_ne!(raster.pixels(), initial, "listbox must visibly paint");
        f.open.revise(|v| *v = false);
        f.host.app.settle(12).unwrap();
        raster.draw(&f.host.renderer).unwrap();
        assert_eq!(raster.pixels(), initial);
        assert_eq!(
            f.host.renderer.engine_id(f.trigger, Part::Main).unwrap(),
            physical
        );
        assert!(f.host.renderer.0.borrow().layers.open.is_empty());
    }
}

#[test]
fn pointer_activation_through_semantic_option_caption_preserves_trigger_focus() {
    let mut f = fixture(ComboboxKind::Editable);
    focus(&mut f.host, f.trigger);
    key(
        &mut f.host,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
    );
    let scope = f.host.app.scope().child_in(f.options[0]).unwrap();
    let reference = scope.cage(None);
    glory_ui::text()
        .content("Caption")
        .node_ref(reference)
        .show_in(&scope)
        .unwrap();
    f.host.app.flush().unwrap();
    settle(&mut f.host);
    let caption = reference.get().unwrap();
    let target = f.host.renderer.engine_id(caption, Part::Main).unwrap();
    let rect = bounds_now(&f.host, caption);
    let (x, y) = (
        (rect.x + rect.width / 2.) as f32,
        (rect.y + rect.height / 2.) as f32,
    );
    let renderer = f.host.renderer.clone();
    EventDriver::new(&mut f.host, Events::new(renderer)).handle_dom_event(DomEvent::new(
        target,
        DomEventData::PointerDown(pointer(x, y, true)),
    ));
    settle(&mut f.host);
    assert_eq!(focused(&f.host), Some(f.trigger));
    let renderer = f.host.renderer.clone();
    EventDriver::new(&mut f.host, Events::new(renderer)).handle_dom_event(DomEvent::new(
        target,
        DomEventData::Click(pointer(x, y, false)),
    ));
    settle(&mut f.host);
    assert_eq!(f.selected.get(), Some(0));
    assert_eq!(focused(&f.host), Some(f.trigger));
}

#[cfg(target_os = "macos")]
#[test]
fn appkit_command_then_raw_key_deletes_once_and_empty_multiple_keeps_physical_modifiers() {
    let mut f = fixture(ComboboxKind::Multiple);
    focus(&mut f.host, f.trigger);
    f.draft.revise(|v| *v = "A".into());
    f.host.app.flush().unwrap();
    key(&mut f.host, Key::Named(NamedKey::End), Modifiers::empty());
    backspace_pair(&mut f.host, Modifiers::empty());
    assert!(f.draft.get().is_empty());
    assert!(
        f.commands.borrow().is_empty(),
        "deleting the last character must not also remove a token"
    );
    backspace_pair(&mut f.host, Modifiers::empty());
    assert_eq!(*f.commands.borrow(), [ChoiceCommand::RemoveLast]);
    backspace_pair(&mut f.host, Modifiers::SHIFT);
    assert_eq!(f.commands.borrow().len(), 1);
    f.host
        .handle_ui_event(UiEvent::AppleStandardKeybinding("deleteBackward:".into()));
    f.host
        .handle_ui_event(UiEvent::PointerMove(pointer(390., 290., false)));
    f.host.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
        key: Key::Named(NamedKey::Backspace),
        code: keyboard_types::Code::Unidentified,
        modifiers: Modifiers::empty(),
        location: keyboard_types::Location::Standard,
        is_auto_repeating: false,
        is_composing: false,
        state: blitz_traits::events::KeyState::Pressed,
        text: None,
    }));
    settle(&mut f.host);
    assert_eq!(
        f.commands.borrow().len(),
        1,
        "another input retires the one pending command"
    );
    f.host
        .handle_ui_event(UiEvent::AppleStandardKeybinding("deleteBackward:".into()));
    assert_eq!(
        f.host.renderer.0.borrow().apple_empty_delete,
        Some(f.trigger)
    );
    f.host.app.clear().unwrap();
    assert!(f.host.renderer.0.borrow().apple_empty_delete.is_none());
    let mut host = WindowApp::new();
    let value = host.app.scope().cage("abc".to_owned());
    host.mount(
        input()
            .purpose(glory_render::Purpose::Password)
            .value(value)
            .on(EventKind::ValueChanged, move |event| {
                if let Event::ValueChanged { value: next, .. } = event {
                    value.revise(|v| *v = next);
                }
            }),
    )
    .unwrap();
    let id = host.app.tree().roots()[0];
    focus(&mut host, id);
    key(&mut host, Key::Named(NamedKey::End), Modifiers::empty());
    backspace_pair(&mut host, Modifiers::empty());
    assert_eq!(value.get(), "ab");
}
