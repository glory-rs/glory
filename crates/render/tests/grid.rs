use glory_render::{memory::Memory, *};
fn grid() -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.role = Some(Role::Grid);
    d.semantics = Some(Box::new(Semantics {
        grid_navigation: Some(Direction::Ltr),
        grid_multiple: true,
        table: Some(TableMetadata::Table {
            rows: Some(6),
            columns: Some(7),
        }),
        ..Default::default()
    }));
    d
}
fn create(
    tree: &mut Memory,
    ids: &mut NodeIds,
    node: Descriptor,
    parent: Option<NodeId>,
) -> NodeId {
    let id = ids.allocate();
    tree.apply(&[
        Mutation::Create {
            id,
            node: Box::new(node),
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
fn grid_metadata_and_selected_are_finite_role_specific_and_roll_back_conflicts() {
    let mut selection_only = Descriptor::new(Props::View);
    selection_only.role = Some(Role::Grid);
    selection_only.semantics = Some(Box::new(Semantics {
        grid_multiple: true,
        ..Default::default()
    }));
    selection_only.validate().unwrap();
    for role in [
        None,
        Some(Role::Table),
        Some(Role::Group),
        Some(Role::GridCell),
    ] {
        let mut bad = selection_only.clone();
        bad.role = role;
        assert!(bad.validate().is_err());
    }
    let d = grid();
    d.validate().unwrap();
    EventKind::GridNavigate.validate_target(&d).unwrap();
    for role in [Role::Table, Role::Group, Role::GridCell] {
        let mut bad = d.clone();
        bad.role = Some(role);
        assert!(bad.validate().is_err());
    }
    let mut cell = Descriptor::new(Props::View);
    cell.role = Some(Role::GridCell);
    cell.semantics = Some(Box::new(Semantics {
        selected: Some(true),
        table: Some(TableMetadata::Cell {
            row: 1,
            column: 2,
            row_span: 1,
            column_span: 1,
            headers: vec![],
        }),
        ..Default::default()
    }));
    cell.validate().unwrap();
    let before = cell.clone();
    assert!(
        cell.patch(Patch::Role(Change::Set(Some(Role::Cell))))
            .is_err()
    );
    assert_eq!(cell, before);
    let mut bad = d.clone();
    bad.semantics.as_mut().unwrap().navigation = Some(Orientation::Horizontal);
    assert!(bad.validate().is_err());
}
#[test]
fn two_dimensional_commands_preserve_ime_editing_modifiers_and_linear_orientation() {
    let key = |name: &str| {
        GridCommand::key(
            Direction::Ltr,
            KeyboardInput {
                key: name,
                ..Default::default()
            },
        )
    };
    assert_eq!(key("ArrowLeft"), Some(GridCommand::Left));
    assert_eq!(key("ArrowDown"), Some(GridCommand::Down));
    assert_eq!(key("Home"), Some(GridCommand::FirstInRow));
    assert_eq!(key("PageDown"), Some(GridCommand::NextPage));
    assert_eq!(
        GridCommand::key(
            Direction::Rtl,
            KeyboardInput {
                key: "ArrowLeft",
                ..Default::default()
            }
        ),
        Some(GridCommand::Right)
    );
    assert_eq!(
        GridCommand::key(
            Direction::Ltr,
            KeyboardInput {
                key: "PageUp",
                shift: true,
                ..Default::default()
            }
        ),
        Some(GridCommand::PreviousLargePage)
    );
    for input in [
        KeyboardInput {
            key: "ArrowLeft",
            composing: true,
            ..Default::default()
        },
        KeyboardInput {
            key: "ArrowLeft",
            editable: true,
            ..Default::default()
        },
        KeyboardInput {
            key: "ArrowLeft",
            control: true,
            ..Default::default()
        },
        KeyboardInput {
            key: "ArrowLeft",
            alt: true,
            ..Default::default()
        },
        KeyboardInput {
            key: "ArrowLeft",
            meta: true,
            ..Default::default()
        },
        KeyboardInput {
            key: "ArrowLeft",
            shift: true,
            ..Default::default()
        },
    ] {
        assert_eq!(GridCommand::key(Direction::Ltr, input), None);
    }
    assert_eq!(
        Orientation::Horizontal.key(KeyboardInput {
            key: "ArrowDown",
            ..Default::default()
        }),
        None
    );
}
#[test]
fn grid_origin_must_be_a_live_enabled_button_in_the_nearest_grid_and_cannot_follow_moves_or_reuse()
{
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let root = create(&mut tree, &mut ids, grid(), None);
    let row = create(
        &mut tree,
        &mut ids,
        Descriptor::new(Props::View),
        Some(root),
    );
    let button = create(
        &mut tree,
        &mut ids,
        Descriptor::new(Props::Button(Default::default())),
        Some(row),
    );
    let event = Event::GridNavigate {
        from: button,
        command: GridCommand::Right,
    };
    assert!(tree.validate_event(root, &event).unwrap());
    let editor = create(
        &mut tree,
        &mut ids,
        Descriptor::new(Props::Input(Default::default())),
        Some(row),
    );
    assert!(
        tree.validate_event(
            root,
            &Event::GridNavigate {
                from: editor,
                command: GridCommand::Left
            }
        )
        .is_err()
    );
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    assert!(matches!(
        tree.validate_event(
            root,
            &Event::GridNavigate {
                from: foreign,
                command: GridCommand::Left
            }
        ),
        Err(Error::ForeignRuntime)
    ));
    let nested = create(&mut tree, &mut ids, grid(), Some(root));
    tree.apply(&[Mutation::Place {
        id: button,
        parent: Some(nested),
        before: None,
    }])
    .unwrap();
    assert!(tree.validate_event(root, &event).is_err());
    assert!(tree.validate_event(nested, &event).unwrap());
    tree.apply(&[Mutation::Place {
        id: button,
        parent: None,
        before: None,
    }])
    .unwrap();
    assert!(tree.validate_event(root, &event).is_err());
    tree.apply(&[
        Mutation::Place {
            id: button,
            parent: Some(root),
            before: None,
        },
        Mutation::Patch {
            id: button,
            patch: Patch::Enabled(Change::Set(false)),
        },
    ])
    .unwrap();
    assert!(!tree.validate_event(root, &event).unwrap());
    tree.apply(&[Mutation::Dispose { id: button }]).unwrap();
    assert!(!tree.validate_event(root, &event).unwrap());
    let new = create(
        &mut tree,
        &mut ids,
        Descriptor::new(Props::Button(Default::default())),
        Some(root),
    );
    assert_ne!(new, button);
    assert!(!tree.validate_event(root, &event).unwrap());
}
