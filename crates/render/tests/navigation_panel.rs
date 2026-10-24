use glory_render::*;
fn trigger() -> Descriptor {
    let mut node = Descriptor::new(Props::Button(ButtonProps {
        label: "Products".into(),
        ..Default::default()
    }));
    node.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("trigger").unwrap()),
        controls: Some(SemanticId::new("content").unwrap()),
        expanded: Some(false),
        navigation_item: true,
        navigation_panel: Some(NavigationPanel::Trigger),
        ..Default::default()
    }));
    node
}
fn content(layer: bool) -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.role = Some(Role::Group);
    node.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("content").unwrap()),
        navigation: Some(Orientation::Vertical),
        navigation_panel: Some(NavigationPanel::Content),
        layer: layer.then(|| Layer {
            modal: false,
            focus: LayerFocus::Preserve,
            placement: LayerPlacement::AnchorStart,
            anchor: Some(SemanticId::new("trigger").unwrap()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    node
}
#[test]
fn navigation_disclosure_and_physical_group_layers_never_claim_menu_or_dialog_roles() {
    trigger().validate().unwrap();
    content(true).validate().unwrap();
    content(false).validate().unwrap();
    for change in 0..7 {
        let mut invalid = content(true);
        match change {
            0 => invalid.props = Props::Group,
            1 => invalid.role = Some(Role::Menu),
            2 => invalid.focus = FocusMode::Programmatic,
            3 => {
                invalid
                    .semantics
                    .as_mut()
                    .unwrap()
                    .layer
                    .as_mut()
                    .unwrap()
                    .modal = true
            }
            4 => {
                invalid
                    .semantics
                    .as_mut()
                    .unwrap()
                    .layer
                    .as_mut()
                    .unwrap()
                    .focus = LayerFocus::Manage
            }
            5 => invalid.semantics.as_mut().unwrap().navigation_panel = None,
            _ => invalid.semantics.as_mut().unwrap().navigation = Some(Orientation::Horizontal),
        }
        assert!(invalid.validate().is_err(), "{change}");
    }
    let mut invalid = trigger();
    invalid.semantics.as_mut().unwrap().popup = Some(PopupKind::Menu);
    assert!(invalid.validate().is_err());
    let mut link = Descriptor::new(Props::Link(LinkProps {
        target: "/docs".into(),
        ..Default::default()
    }));
    link.semantics = Some(Box::new(Semantics {
        navigation_item: true,
        ..Default::default()
    }));
    link.validate().unwrap();
    link.semantics.as_mut().unwrap().selected = Some(true);
    assert!(link.validate().is_err());
}
#[test]
fn navigation_commands_follow_the_actual_mounted_level_and_exact_origin() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let header = ids.allocate();
    let panel = ids.allocate();
    let nested = ids.allocate();
    let link = ids.allocate();
    let mut memory = memory::Memory::new(runtime);
    let mut nested_node = trigger();
    nested_node.semantics.as_mut().unwrap().id = Some(SemanticId::new("nested-trigger").unwrap());
    let mut leaf = Descriptor::new(Props::Link(LinkProps {
        target: "/docs".into(),
        ..Default::default()
    }));
    leaf.semantics = Some(Box::new(Semantics {
        navigation_item: true,
        ..Default::default()
    }));
    memory
        .apply(&[
            Mutation::Create {
                id: header,
                node: Box::new(trigger()),
            },
            Mutation::Place {
                id: header,
                parent: None,
                before: None,
            },
            Mutation::Create {
                id: panel,
                node: Box::new(content(false)),
            },
            Mutation::Place {
                id: panel,
                parent: None,
                before: None,
            },
            Mutation::Create {
                id: nested,
                node: Box::new(nested_node),
            },
            Mutation::Place {
                id: nested,
                parent: Some(panel),
                before: None,
            },
            Mutation::Create {
                id: link,
                node: Box::new(leaf),
            },
            Mutation::Place {
                id: link,
                parent: Some(panel),
                before: None,
            },
        ])
        .unwrap();
    for (command, node) in [
        (NavigationPanelCommand::First, header),
        (NavigationPanelCommand::Last, header),
        (NavigationPanelCommand::Open, nested),
        (NavigationPanelCommand::Close, nested),
        (NavigationPanelCommand::Close, link),
    ] {
        assert!(
            memory
                .validate_event(
                    node,
                    &Event::NavigationPanel {
                        from: node,
                        command
                    }
                )
                .unwrap()
        );
    }
    for (command, node) in [
        (NavigationPanelCommand::Close, header),
        (NavigationPanelCommand::Open, header),
        (NavigationPanelCommand::First, nested),
        (NavigationPanelCommand::First, link),
    ] {
        assert!(
            memory
                .validate_event(
                    node,
                    &Event::NavigationPanel {
                        from: node,
                        command
                    }
                )
                .is_err()
        );
    }
    assert!(
        memory
            .validate_event(
                header,
                &Event::NavigationPanel {
                    from: nested,
                    command: NavigationPanelCommand::First
                }
            )
            .is_err()
    );
}
#[test]
fn keyboard_navigation_panel_commands_preserve_tab_editors_modifiers_and_parallel_arrows() {
    for (direction, forward, back) in [
        (Direction::Ltr, "ArrowRight", "ArrowLeft"),
        (Direction::Rtl, "ArrowLeft", "ArrowRight"),
    ] {
        for (key, trigger, inside, expected) in [
            (
                "ArrowDown",
                true,
                false,
                Some(NavigationPanelCommand::First),
            ),
            ("ArrowUp", true, false, Some(NavigationPanelCommand::Last)),
            (forward, true, true, Some(NavigationPanelCommand::Open)),
            (back, false, true, Some(NavigationPanelCommand::Close)),
            ("Tab", false, true, None),
            ("Enter", true, false, None),
            ("ArrowDown", true, true, None),
        ] {
            let input = KeyboardInput {
                key,
                ..Default::default()
            };
            assert_eq!(input.navigation_panel(direction, trigger, inside), expected);
            for blocked in [
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
                assert_eq!(blocked.navigation_panel(direction, trigger, inside), None);
            }
        }
    }
}
