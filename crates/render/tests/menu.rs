use glory_render::memory::Memory;
use glory_render::*;
fn panel() -> Descriptor {
    let mut panel = Descriptor::new(Props::View);
    panel.role = Some(Role::Menu);
    panel.focus = FocusMode::Programmatic;
    panel.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("menu").unwrap()),
        navigation: Some(Orientation::Vertical),
        typeahead: true,
        layer: Some(Layer {
            modal: false,
            placement: LayerPlacement::AnchorRight,
            anchor: Some(SemanticId::new("trigger").unwrap()),
            return_focus: Some(SemanticId::new("trigger").unwrap()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    panel
}
#[test]
fn menu_layer_and_item_refinements_validate_their_actual_intrinsic_controls() {
    let menu = panel();
    menu.validate().unwrap();
    Action::ExitMenu {
        node: NodeIds::new(RuntimeId::new()).allocate(),
        backwards: false,
    }
    .validate_target(&menu)
    .unwrap();
    let mut invalid = menu.clone();
    invalid
        .semantics
        .as_mut()
        .unwrap()
        .layer
        .as_mut()
        .unwrap()
        .modal = true;
    assert!(invalid.validate().is_err());
    invalid = menu.clone();
    invalid.semantics.as_mut().unwrap().navigation = Some(Orientation::Horizontal);
    assert!(invalid.validate().is_err());
    for (kind, props) in [
        (MenuItemKind::Action, Props::Button(ButtonProps::default())),
        (MenuItemKind::Checkbox, Props::Check(CheckProps::default())),
        (
            MenuItemKind::Radio,
            Props::Check(CheckProps {
                mode: CheckMode::Radio,
                ..Default::default()
            }),
        ),
    ] {
        let mut descriptor = Descriptor::new(props);
        descriptor.semantics = Some(Box::new(Semantics {
            menu_item: Some(MenuItemSemantics {
                kind,
                owner: SemanticId::new("menu").unwrap(),
            }),
            check_group: (kind == MenuItemKind::Radio).then(|| SemanticId::new("radios").unwrap()),
            navigation_item: true,
            ..Default::default()
        }));
        descriptor.validate().unwrap();
        let before = descriptor.clone();
        assert!(
            descriptor
                .patch(Patch::Role(Change::Set(Some(Role::Menu))))
                .is_err()
        );
        assert_eq!(descriptor, before);
        if kind == MenuItemKind::Radio {
            assert!(descriptor.patch(Patch::CheckMode(Change::Clear)).is_err());
            assert_eq!(descriptor, before);
        }
        let mut metadata = descriptor.semantics.clone().unwrap();
        metadata.form_control = Some(FormControl {
            name: "business".into(),
            value: None,
        });
        assert!(
            descriptor
                .patch(Patch::Semantics(Change::Set(Some(metadata))))
                .is_err()
        );
        assert_eq!(descriptor, before);
    }
    let mut view = Descriptor::new(Props::View);
    view.semantics = Some(Box::new(Semantics {
        menu_item: Some(MenuItemSemantics {
            kind: MenuItemKind::Action,
            owner: SemanticId::new("menu").unwrap(),
        }),
        ..Default::default()
    }));
    assert!(view.validate().is_err());
}
#[test]
fn pointer_related_identity_ancestry_retirement_and_direct_queue_rules_agree() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let parent = ids.allocate();
    let item = ids.allocate();
    let nested = ids.allocate();
    let sibling = ids.allocate();
    let group = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        Mutation::Create {
            id: parent,
            node: Box::new(Descriptor::new(Props::View)),
        },
        Mutation::Place {
            id: parent,
            parent: None,
            before: None,
        },
        Mutation::Create {
            id: item,
            node: Box::new(Descriptor::new(Props::View)),
        },
        Mutation::Place {
            id: item,
            parent: Some(parent),
            before: None,
        },
        Mutation::Create {
            id: nested,
            node: Box::new(Descriptor::new(Props::View)),
        },
        Mutation::Place {
            id: nested,
            parent: Some(item),
            before: None,
        },
        Mutation::Create {
            id: sibling,
            node: Box::new(Descriptor::new(Props::View)),
        },
        Mutation::Place {
            id: sibling,
            parent: Some(parent),
            before: None,
        },
        Mutation::Create {
            id: group,
            node: Box::new(Descriptor::new(Props::Group)),
        },
        Mutation::Place {
            id: group,
            parent: Some(parent),
            before: None,
        },
    ])
    .unwrap();
    let event = |related| Event::Pointer {
        from: item,
        related,
        phase: PointerPhase::Leave,
    };
    assert!(!tree.validate_event(item, &event(Some(item))).unwrap());
    assert!(!tree.validate_event(item, &event(Some(nested))).unwrap());
    assert!(tree.validate_event(item, &event(Some(sibling))).unwrap());
    assert!(tree.validate_event(item, &event(Some(parent))).unwrap());
    assert!(tree.validate_event(item, &event(Some(group))).is_err());
    assert!(tree.validate_event(sibling, &event(Some(parent))).is_err());
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    assert_eq!(
        tree.validate_event(item, &event(Some(foreign))),
        Err(Error::ForeignRuntime)
    );
    tree.push_event(item, event(Some(sibling))).unwrap();
    tree.apply(&[Mutation::Dispose { id: sibling }]).unwrap();
    let mut inputs = Vec::new();
    tree.drain(&mut inputs);
    assert!(inputs.is_empty());
    assert!(!tree.validate_event(item, &event(Some(sibling))).unwrap());
    tree.apply(&[Mutation::Suspend { id: nested }]).unwrap();
    assert!(!tree.validate_event(item, &event(Some(nested))).unwrap());
    assert!(
        EventKind::Pointer
            .validate_target(&Descriptor::new(Props::Group))
            .is_err()
    );
}
#[test]
fn menu_keyboard_preserves_editing_composition_chords_and_has_explicit_rtl_tab_commands() {
    let input = KeyboardInput {
        key: "ArrowRight",
        ..Default::default()
    };
    assert_eq!(input.menu(Direction::Ltr, false), Some(MenuCommand::Open));
    assert_eq!(input.menu(Direction::Rtl, false), Some(MenuCommand::Close));
    for input in [
        KeyboardInput {
            editable: true,
            ..input
        },
        KeyboardInput {
            composing: true,
            ..input
        },
        KeyboardInput {
            control: true,
            ..input
        },
        KeyboardInput {
            meta: true,
            ..input
        },
        KeyboardInput { alt: true, ..input },
        KeyboardInput {
            shift: true,
            ..input
        },
    ] {
        assert_eq!(input.menu(Direction::Ltr, false), None);
    }
    assert_eq!(
        KeyboardInput {
            key: "ArrowUp",
            ..Default::default()
        }
        .menu(Direction::Rtl, true),
        Some(MenuCommand::Last)
    );
    assert_eq!(
        KeyboardInput {
            key: "Tab",
            shift: true,
            ..Default::default()
        }
        .menu(Direction::Ltr, false),
        Some(MenuCommand::Exit { backwards: true })
    );
}
