use glory_render::{memory::Memory, *};
fn root() -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.role = Some(Role::Tree);
    d.semantics = Some(Box::new(Semantics {
        tree_navigation: Some(TreeNavigation::default()),
        ..Default::default()
    }));
    d
}
fn item() -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.role = Some(Role::TreeItem);
    d.semantics = Some(Box::new(Semantics {
        tree_item: Some(TreeItem {
            level: 2,
            position: 1,
            count: 3,
        }),
        selected: Some(true),
        expanded: Some(false),
        ..Default::default()
    }));
    d
}
#[test]
fn checked_items_require_coordinates_and_are_independent_from_selected() {
    for state in [
        TreeCheckState::Unchecked,
        TreeCheckState::Checked,
        TreeCheckState::Mixed,
    ] {
        let mut d = item();
        let s = d.semantics.as_mut().unwrap();
        s.selected = None;
        s.tree_checked = Some(state);
        d.validate().unwrap();
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let mut tree = Memory::new(runtime);
        let id = add(&mut tree, &mut ids, d.clone(), None);
        let before = tree.node(id).unwrap().descriptor.to_owned();
        let stats = tree.stats();
        let mut bad = d.semantics.clone().unwrap();
        bad.selected = Some(false);
        assert!(
            tree.prepare_patch(id, &Patch::Semantics(Change::Set(Some(bad))))
                .is_err()
        );
        assert_eq!(tree.stats(), stats);
        assert_eq!(tree.node(id).unwrap().descriptor.to_owned(), before);
        d.semantics.as_mut().unwrap().tree_item = None;
        assert!(d.validate().is_err());
    }
    let mut d = root();
    d.semantics.as_mut().unwrap().tree_checked = Some(TreeCheckState::Mixed);
    assert!(d.validate().is_err());
}
#[test]
fn tree_popup_requires_intrinsic_select_and_a_managed_nonmodal_tree() {
    let mut d = Descriptor::new(Props::Button(Default::default()));
    d.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("trigger").unwrap()),
        controls: Some(SemanticId::new("tree").unwrap()),
        combobox: Some(ComboboxKind::Select),
        popup: Some(PopupKind::Tree),
        expanded: Some(true),
        ..Default::default()
    }));
    d.validate().unwrap();
    d.semantics.as_mut().unwrap().active_descendant = Some(SemanticId::new("item").unwrap());
    assert!(d.validate().is_err());
    d.semantics.as_mut().unwrap().active_descendant = None;
    d.props = Props::Input(Default::default());
    d.semantics.as_mut().unwrap().combobox = Some(ComboboxKind::Editable);
    assert!(d.validate().is_err());
    let mut layer = root();
    layer.focus = FocusMode::Programmatic;
    layer.semantics.as_mut().unwrap().id = Some(SemanticId::new("tree").unwrap());
    layer.semantics.as_mut().unwrap().layer = Some(Layer {
        modal: false,
        ..Default::default()
    });
    layer.validate().unwrap();
    layer
        .semantics
        .as_mut()
        .unwrap()
        .layer
        .as_mut()
        .unwrap()
        .modal = true;
    assert!(layer.validate().is_err());
    layer
        .semantics
        .as_mut()
        .unwrap()
        .layer
        .as_mut()
        .unwrap()
        .modal = false;
    layer
        .semantics
        .as_mut()
        .unwrap()
        .layer
        .as_mut()
        .unwrap()
        .focus = LayerFocus::Preserve;
    assert!(layer.validate().is_err());
}
#[test]
fn prepared_combobox_reference_accepts_the_declared_tree_and_rejects_listbox_substitution() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let mut owner = root();
    owner.semantics.as_mut().unwrap().id = Some(SemanticId::new("popup").unwrap());
    add(&mut memory, &mut ids, owner, None);
    let mut trigger = Descriptor::new(Props::Button(Default::default()));
    trigger.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("trigger").unwrap()),
        controls: Some(SemanticId::new("popup").unwrap()),
        combobox: Some(ComboboxKind::Select),
        popup: Some(PopupKind::Tree),
        expanded: Some(false),
        ..Default::default()
    }));
    let id = add(&mut memory, &mut ids, trigger.clone(), None);
    let mut next = trigger.semantics.clone().unwrap();
    next.expanded = Some(true);
    let patch = Patch::Semantics(Change::Set(Some(next.clone())));
    memory.prepare_patch(id, &patch).unwrap().commit();
    let before = memory.node(id).unwrap().descriptor.to_owned();
    let stats = memory.stats();
    next.popup = Some(PopupKind::Listbox);
    assert!(
        memory
            .prepare_patch(id, &Patch::Semantics(Change::Set(Some(next))))
            .is_err()
    );
    assert_eq!(memory.stats(), stats);
    assert_eq!(memory.node(id).unwrap().descriptor.to_owned(), before);
}
fn add(tree: &mut Memory, ids: &mut NodeIds, d: Descriptor, parent: Option<NodeId>) -> NodeId {
    let id = ids.allocate();
    tree.apply(&[
        Mutation::Create {
            id,
            node: Box::new(d),
        },
        Mutation::Place {
            id,
            parent,
            before: None,
        },
    ])
    .unwrap();
    id
}
#[test]
fn independent_tree_coordinates_and_fields_validate_and_prepared_patch_rolls_back() {
    root().validate().unwrap();
    item().validate().unwrap();
    for data in [
        TreeItem {
            level: 0,
            position: 0,
            count: 1,
        },
        TreeItem {
            level: 65,
            position: 0,
            count: 1,
        },
        TreeItem {
            level: 1,
            position: 0,
            count: 0,
        },
        TreeItem {
            level: 1,
            position: 2,
            count: 2,
        },
    ] {
        let mut d = item();
        d.semantics.as_mut().unwrap().tree_item = Some(data);
        assert!(d.validate().is_err());
    }
    for role in [Role::GridCell, Role::Group, Role::Tree] {
        let mut d = item();
        d.role = Some(role);
        assert!(d.validate().is_err());
    }
    for role in [Role::Grid, Role::Group, Role::TreeItem] {
        let mut d = root();
        d.role = Some(role);
        assert!(d.validate().is_err());
    }
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let id = add(&mut tree, &mut ids, item(), None);
    let before = tree.node(id).unwrap().descriptor.to_owned();
    let stats = tree.stats();
    assert!(
        tree.prepare_patch(id, &Patch::Role(Change::Set(Some(Role::Group))))
            .is_err()
    );
    assert_eq!(tree.node(id).unwrap().descriptor.to_owned(), before);
    assert_eq!(tree.stats(), stats);
    let mut d = root();
    d.semantics.as_mut().unwrap().grid_navigation = Some(Direction::Ltr);
    assert!(d.validate().is_err());
}
#[test]
fn tree_commands_keep_physical_origin_and_nearest_live_enabled_tree() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let root = add(&mut tree, &mut ids, root(), None);
    let group = add(
        &mut tree,
        &mut ids,
        Descriptor::new(Props::Group),
        Some(root),
    );
    let id = add(&mut tree, &mut ids, item(), Some(group));
    let event = Event::TreeNavigate {
        from: id,
        command: TreeCommand::Open,
    };
    assert!(tree.validate_event(root, &event).unwrap());
    let editor = add(
        &mut tree,
        &mut ids,
        Descriptor::new(Props::Input(Default::default())),
        Some(id),
    );
    assert!(
        tree.validate_event(
            root,
            &Event::TreeNavigate {
                from: editor,
                command: TreeCommand::Open
            }
        )
        .is_err()
    );
    let nested = add(
        &mut tree,
        &mut ids,
        {
            let mut d = Descriptor::new(Props::View);
            d.role = Some(Role::Tree);
            d.semantics = Some(Box::new(Semantics {
                tree_navigation: Some(TreeNavigation::default()),
                ..Default::default()
            }));
            d
        },
        Some(root),
    );
    tree.apply(&[Mutation::Place {
        id,
        parent: Some(nested),
        before: None,
    }])
    .unwrap();
    assert!(tree.validate_event(root, &event).is_err());
    assert!(tree.validate_event(nested, &event).unwrap());
    let mut metadata = item().semantics.unwrap();
    metadata.disabled = true;
    tree.apply(&[Mutation::Patch {
        id,
        patch: Patch::Semantics(Change::Set(Some(metadata))),
    }])
    .unwrap();
    assert!(!tree.validate_event(nested, &event).unwrap());
    tree.apply(&[Mutation::Dispose { id }]).unwrap();
    assert!(!tree.validate_event(nested, &event).unwrap());
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    assert!(matches!(
        tree.validate_event(
            root,
            &Event::TreeNavigate {
                from: foreign,
                command: TreeCommand::Open
            }
        ),
        Err(Error::ForeignRuntime)
    ));
}
#[test]
fn tree_keyboard_is_directional_bounded_and_does_not_capture_editors_or_chords() {
    let config = TreeNavigation::default();
    let input = KeyboardInput {
        key: "ArrowRight",
        ..Default::default()
    };
    assert_eq!(TreeCommand::key(config, input, 1), Some(TreeCommand::Open));
    assert_eq!(
        TreeCommand::key(
            TreeNavigation {
                direction: Direction::Rtl,
                ..config
            },
            input,
            1
        ),
        Some(TreeCommand::Close)
    );
    for input in [
        KeyboardInput {
            composing: true,
            ..input
        },
        KeyboardInput {
            editable: true,
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
        assert_eq!(TreeCommand::key(config, input, 1), None);
    }
    assert!(
        TreeCommand::Search {
            text: "ab".into(),
            at_millis: 1
        }
        .validate()
        .is_err()
    );
    assert!(
        TreeCommand::Search {
            text: "a".into(),
            at_millis: u64::MAX
        }
        .validate()
        .is_err()
    );
    assert_eq!(
        TreeCommand::key(
            config,
            KeyboardInput {
                key: " ",
                repeat: true,
                ..Default::default()
            },
            1
        ),
        None
    );
    assert_eq!(
        TreeCommand::key(
            config,
            KeyboardInput {
                key: "文",
                ..Default::default()
            },
            7
        ),
        Some(TreeCommand::Search {
            text: "文".into(),
            at_millis: 7
        })
    );
}
