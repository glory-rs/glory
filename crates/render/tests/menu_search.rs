use glory_render::{memory::Memory, *};

fn name(value: &str) -> SemanticId {
    SemanticId::new(value).unwrap()
}
fn search() -> Descriptor {
    let mut node = Descriptor::new(Props::Input(InputProps {
        purpose: Purpose::Search,
        ..Default::default()
    }));
    node.name = Some("Search actions".into());
    node.semantics = Some(Box::new(Semantics {
        id: Some(name("search")),
        controls: Some(name("menu")),
        menu_search: Some(name("menu")),
        ..Default::default()
    }));
    node
}
fn panel(value: &str) -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.role = Some(Role::Menu);
    node.name = Some(value.into());
    node.semantics = Some(Box::new(Semantics {
        id: Some(name(value)),
        ..Default::default()
    }));
    node
}
fn create(id: NodeId, node: Descriptor) -> Mutation {
    Mutation::Create {
        id,
        node: Box::new(node),
    }
}
fn place(id: NodeId, parent: Option<NodeId>) -> Mutation {
    Mutation::Place {
        id,
        parent,
        before: None,
    }
}

#[test]
fn menu_search_requires_a_distinct_menu_relation_and_independent_search_editor() {
    search().validate().unwrap();
    for props in [
        Props::Button(ButtonProps::default()),
        Props::Input(InputProps::default()),
        Props::Input(InputProps {
            purpose: Purpose::Search,
            multiline: true,
            ..Default::default()
        }),
    ] {
        let mut node = search();
        node.props = props;
        assert!(node.validate().is_err());
    }
    for metadata in [
        Semantics {
            controls: None,
            ..*search().semantics.unwrap()
        },
        Semantics {
            popup: Some(PopupKind::Menu),
            ..*search().semantics.unwrap()
        },
        Semantics {
            expanded: Some(true),
            ..*search().semantics.unwrap()
        },
        Semantics {
            id: Some(name("menu")),
            ..*search().semantics.unwrap()
        },
    ] {
        let mut node = search();
        node.semantics = Some(Box::new(metadata));
        assert!(node.validate().is_err());
    }
    let target = NodeIds::new(RuntimeId::new()).allocate();
    let event = Event::Menu {
        from: target,
        command: MenuCommand::SearchNext,
    };
    event.validate_payload(target, &search()).unwrap();
    assert!(event.validate_payload(target, &panel("menu")).is_err());
    assert!(
        Event::Menu {
            from: target,
            command: MenuCommand::Open
        }
        .validate_payload(target, &search())
        .is_err()
    );
}

#[test]
fn menu_search_keyboard_preserves_caret_shortcuts_ime_and_enter_repeat() {
    for (key, expected) in [
        ("ArrowDown", MenuCommand::SearchNext),
        ("ArrowUp", MenuCommand::SearchPrevious),
        ("Enter", MenuCommand::SearchCommit),
        ("Tab", MenuCommand::Exit { backwards: false }),
    ] {
        let input = KeyboardInput {
            key,
            editable: true,
            ..Default::default()
        };
        assert_eq!(input.menu_search(), Some(expected));
        for modified in [
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
        ] {
            assert_eq!(modified.menu_search(), None);
        }
    }
    for key in ["ArrowLeft", "ArrowRight", "Home", "End", " ", "a", "Escape"] {
        assert_eq!(
            KeyboardInput {
                key,
                editable: true,
                ..Default::default()
            }
            .menu_search(),
            None
        );
    }
    assert_eq!(
        KeyboardInput {
            key: "Tab",
            shift: true,
            ..Default::default()
        }
        .menu_search(),
        Some(MenuCommand::Exit { backwards: true })
    );
    assert_eq!(
        KeyboardInput {
            key: "Enter",
            repeat: true,
            ..Default::default()
        }
        .menu_search(),
        None
    );
}

#[test]
fn search_queries_and_queued_commands_use_current_menu_ancestry_and_availability() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let menu = ids.allocate();
    let other = ids.allocate();
    let input = ids.allocate();
    let item = ids.allocate();
    let mut action = Descriptor::new(Props::Button(ButtonProps {
        label: "Save".into(),
        ..Default::default()
    }));
    action.semantics = Some(Box::new(Semantics {
        id: Some(name("save")),
        menu_item: Some(MenuItemSemantics {
            kind: MenuItemKind::Action,
            owner: name("menu"),
        }),
        ..Default::default()
    }));
    let mut editor = search();
    editor.semantics.as_mut().unwrap().active_descendant = Some(name("save"));
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        create(menu, panel("menu")),
        place(menu, None),
        create(other, panel("other")),
        place(other, None),
        create(item, action),
        place(item, Some(menu)),
        create(input, editor),
        place(input, Some(menu)),
    ])
    .unwrap();
    assert_eq!(tree.menu_search_owner(input).unwrap(), Some(menu));
    assert_eq!(tree.resolve_active_descendant(input).unwrap(), Some(item));
    let event = Event::Menu {
        from: input,
        command: MenuCommand::SearchCommit,
    };
    tree.push_event(input, event.clone()).unwrap();
    tree.apply(&[place(input, Some(other))]).unwrap();
    assert_eq!(tree.menu_search_owner(input).unwrap(), None);
    assert_eq!(tree.resolve_active_descendant(input).unwrap(), None);
    assert!(!tree.validate_event(input, &event).unwrap());
    let mut pending = Vec::new();
    tree.drain(&mut pending);
    assert!(pending.is_empty());
    tree.apply(&[place(input, Some(menu)), Mutation::Suspend { id: item }])
        .unwrap();
    assert_eq!(tree.resolve_active_descendant(input).unwrap(), None);
    tree.apply(&[Mutation::Resume {
        id: item,
        parent: Some(menu),
        before: None,
    }])
    .unwrap();
    assert_eq!(tree.resolve_active_descendant(input).unwrap(), Some(item));
    tree.apply(&[Mutation::Suspend { id: menu }]).unwrap();
    assert_eq!(tree.resolve_active_descendant(input).unwrap(), None);
    assert!(!tree.validate_event(input, &event).unwrap());
}
