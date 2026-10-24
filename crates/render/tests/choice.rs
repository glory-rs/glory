use glory_render::{memory::Memory, *};

fn name(value: &str) -> SemanticId {
    SemanticId::new(value).unwrap()
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
fn listbox(value: &str) -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.role = Some(Role::Listbox);
    d.semantics = Some(Box::new(Semantics {
        id: Some(name(value)),
        navigation: Some(Orientation::Vertical),
        ..Default::default()
    }));
    d
}
fn option(value: &str, owner: &str) -> Descriptor {
    let mut d = Descriptor::new(Props::Button(ButtonProps {
        label: value.into(),
        ..Default::default()
    }));
    d.focus = FocusMode::Programmatic;
    d.semantics = Some(Box::new(Semantics {
        id: Some(name(value)),
        option: Some(OptionSemantics { owner: name(owner) }),
        selected: Some(false),
        ..Default::default()
    }));
    d
}
fn combo(active: Option<&str>) -> Descriptor {
    let mut d = Descriptor::new(Props::Button(ButtonProps::default()));
    d.semantics = Some(Box::new(Semantics {
        id: Some(name("combo")),
        combobox: Some(ComboboxKind::Select),
        popup: Some(PopupKind::Listbox),
        controls: Some(name("box")),
        expanded: Some(true),
        active_descendant: active.map(name),
        typeahead: true,
        ..Default::default()
    }));
    d
}

#[test]
fn intrinsic_refinements_and_commands_preserve_text_composition_and_tab_defaults() {
    combo(None).validate().unwrap();
    option("one", "box").validate().unwrap();
    listbox("box").validate().unwrap();
    let mut invalid = combo(None);
    invalid.props = Props::View;
    assert!(invalid.validate().is_err());
    let mut invalid = option("one", "box");
    invalid.focus = FocusMode::TabStop;
    assert!(invalid.validate().is_err());
    let input = KeyboardInput {
        key: "ArrowDown",
        ..Default::default()
    };
    assert_eq!(
        input.choice(ComboboxKind::Select, false),
        Some(ChoiceCommand::Next)
    );
    assert_eq!(
        KeyboardInput {
            key: "Home",
            editable: true,
            ..input
        }
        .choice(ComboboxKind::Editable, true),
        None
    );
    assert_eq!(
        KeyboardInput {
            key: "Tab",
            ..input
        }
        .choice(ComboboxKind::Select, true),
        None
    );
    assert_eq!(
        KeyboardInput {
            composing: true,
            ..input
        }
        .choice(ComboboxKind::Editable, true),
        None
    );
    assert_eq!(
        KeyboardInput {
            key: " ",
            editable: true,
            ..input
        }
        .choice(ComboboxKind::Editable, true),
        None
    );
}

#[test]
fn autocomplete_and_multiple_are_intrinsic_inputs_with_finite_remove_last_commands() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    for kind in [
        ComboboxKind::Select,
        ComboboxKind::Editable,
        ComboboxKind::Autocomplete,
        ComboboxKind::Multiple,
    ] {
        let mut descriptor = combo(None);
        if kind != ComboboxKind::Select {
            descriptor.props = Props::Input(InputProps::default());
        }
        descriptor.semantics.as_mut().unwrap().combobox = Some(kind);
        descriptor.semantics.as_mut().unwrap().typeahead = kind == ComboboxKind::Select;
        descriptor.validate().unwrap();
        let id = ids.allocate();
        let event = Event::Choice {
            from: id,
            command: ChoiceCommand::RemoveLast,
        };
        assert_eq!(
            event.validate_payload(id, descriptor.view()).is_ok(),
            kind == ComboboxKind::Multiple
        );
        let key = KeyboardInput {
            key: "Backspace",
            ..Default::default()
        };
        assert_eq!(
            key.choice_remove_last(kind, true),
            (kind == ComboboxKind::Multiple).then_some(ChoiceCommand::RemoveLast)
        );
        assert_eq!(key.choice_remove_last(kind, false), None);
        assert_eq!(
            KeyboardInput {
                composing: true,
                ..key
            }
            .choice_remove_last(kind, true),
            None
        );
        assert_eq!(
            KeyboardInput {
                control: true,
                ..key
            }
            .choice_remove_last(kind, true),
            None
        );
        if kind != ComboboxKind::Select {
            for key in ["Home", "End", " ", "Backspace"] {
                assert_eq!(
                    KeyboardInput {
                        key,
                        ..Default::default()
                    }
                    .choice(kind, true),
                    None
                );
            }
        }
    }
}

#[test]
fn multiple_known_owner_must_match_but_late_missing_hidden_and_retired_relations_are_safe() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let trigger = ids.allocate();
    let owner = ids.allocate();
    let mut d = combo(None);
    d.props = Props::Input(InputProps::default());
    d.semantics.as_mut().unwrap().combobox = Some(ComboboxKind::Multiple);
    d.semantics.as_mut().unwrap().typeahead = false;
    let mut tree = Memory::new(runtime);
    tree.apply(&[create(trigger, d.clone()), place(trigger, None)])
        .unwrap();
    let remove = Event::Choice {
        from: trigger,
        command: ChoiceCommand::RemoveLast,
    };
    assert_eq!(tree.validate_event(trigger, &remove), Ok(false));
    tree.apply(&[create(owner, listbox("box")), place(owner, None)])
        .unwrap();
    assert!(tree.validate_event(trigger, &remove).is_err());
    assert!(
        tree.apply(&[Mutation::Patch {
            id: trigger,
            patch: Patch::Semantics(Change::Set(d.semantics.clone()))
        }])
        .is_err()
    );
    let mut multiple = listbox("box").semantics.unwrap();
    multiple.listbox_multiple = true;
    multiple.hidden = true;
    tree.apply(&[Mutation::Patch {
        id: owner,
        patch: Patch::Semantics(Change::Set(Some(multiple))),
    }])
    .unwrap();
    // A closed popup does not prevent removing an already selected tag.
    assert_eq!(tree.validate_event(trigger, &remove), Ok(true));
    tree.apply(&[Mutation::Dispose { id: owner }]).unwrap();
    assert_eq!(tree.validate_event(trigger, &remove), Ok(false));
}

#[test]
fn choice_origins_are_exact_and_retired_slots_do_not_route_to_replacements() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let trigger = ids.allocate();
    let plain = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        create(trigger, combo(None)),
        place(trigger, None),
        create(
            plain,
            Descriptor::new(Props::Button(ButtonProps::default())),
        ),
        place(plain, None),
    ])
    .unwrap();
    let command = ChoiceCommand::OpenFirst;
    assert_eq!(
        tree.validate_event(
            trigger,
            &Event::Choice {
                from: trigger,
                command
            }
        ),
        Ok(true)
    );
    assert!(
        tree.validate_event(
            trigger,
            &Event::Choice {
                from: plain,
                command
            }
        )
        .is_err()
    );
    assert!(
        tree.validate_event(
            plain,
            &Event::Choice {
                from: plain,
                command
            }
        )
        .is_err()
    );
    tree.apply(&[Mutation::Suspend { id: trigger }]).unwrap();
    assert!(
        tree.push_event(
            trigger,
            Event::Choice {
                from: trigger,
                command
            }
        )
        .is_err()
    );
    tree.apply(&[Mutation::Dispose { id: trigger }]).unwrap();
    assert!(ids.release(trigger));
    let replacement = ids.allocate();
    tree.apply(&[create(replacement, combo(None)), place(replacement, None)])
        .unwrap();
    assert_ne!(replacement, trigger);
    assert!(
        tree.validate_event(
            replacement,
            &Event::Choice {
                from: trigger,
                command
            }
        )
        .is_err()
    );
}

#[test]
fn active_relation_resolves_late_and_disposal_before_reference_cleanup_is_valid() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let trigger = ids.allocate();
    let box_id = ids.allocate();
    let item = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[create(trigger, combo(Some("one"))), place(trigger, None)])
        .unwrap();
    assert_eq!(tree.resolve_active_descendant(trigger).unwrap(), None);
    tree.apply(&[
        create(box_id, listbox("box")),
        place(box_id, None),
        create(item, option("one", "box")),
    ])
    .unwrap();
    assert_eq!(tree.resolve_active_descendant(trigger).unwrap(), None);
    tree.apply(&[place(item, Some(box_id))]).unwrap();
    assert_eq!(tree.resolve_active_descendant(trigger).unwrap(), Some(item));
    tree.apply(&[Mutation::Suspend { id: item }]).unwrap();
    assert_eq!(tree.resolve_active_descendant(trigger).unwrap(), None);
    assert!(
        tree.apply(&[Mutation::Resume {
            id: item,
            parent: None,
            before: None
        }])
        .is_err()
    );
    tree.apply(&[Mutation::Resume {
        id: item,
        parent: Some(box_id),
        before: None,
    }])
    .unwrap();
    tree.apply(&[Mutation::Dispose { id: item }]).unwrap();
    assert_eq!(tree.resolve_active_descendant(trigger).unwrap(), None);
    let replacement = ids.allocate();
    tree.apply(&[
        create(replacement, option("one", "box")),
        place(replacement, Some(box_id)),
    ])
    .unwrap();
    assert_eq!(
        tree.resolve_active_descendant(trigger).unwrap(),
        Some(replacement)
    );
    let mut disabled = option("one", "box").semantics.unwrap();
    disabled.disabled = true;
    tree.apply(&[Mutation::Patch {
        id: replacement,
        patch: Patch::Semantics(Change::Set(Some(disabled))),
    }])
    .unwrap();
    assert_eq!(tree.resolve_active_descendant(trigger).unwrap(), None);
}

#[test]
fn option_ownership_survives_group_move_suspend_and_same_owner_reorder() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let a = ids.allocate();
    let b = ids.allocate();
    let group = ids.allocate();
    let item = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        create(a, listbox("box")),
        place(a, None),
        create(b, listbox("other")),
        place(b, None),
        create(group, Descriptor::new(Props::Group)),
        place(group, Some(a)),
        create(item, option("one", "box")),
        place(item, Some(group)),
    ])
    .unwrap();
    let stats = tree.stats();
    assert!(tree.apply(&[place(group, None)]).is_err());
    assert_eq!(tree.stats(), stats);
    assert_eq!(tree.node(group).unwrap().parent, Some(a));
    assert!(tree.apply(&[place(group, Some(b))]).is_err());
    tree.apply(&[place(group, Some(a)), Mutation::Suspend { id: group }])
        .unwrap();
    assert!(
        tree.apply(&[Mutation::Resume {
            id: group,
            parent: Some(b),
            before: None
        }])
        .is_err()
    );
    tree.apply(&[Mutation::Resume {
        id: group,
        parent: Some(a),
        before: None,
    }])
    .unwrap();
    let stats = tree.stats();
    assert!(
        tree.prepare_patch(item, &Patch::Semantics(Change::Set(None)))
            .is_err()
    );
    assert_eq!(tree.stats(), stats);
}

#[test]
fn filtered_hidden_options_are_inactive_without_losing_structural_owner_validation() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let owner = ids.allocate();
    let item = ids.allocate();
    let trigger = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        create(owner, listbox("box")),
        place(owner, None),
        create(item, option("one", "box")),
        place(item, Some(owner)),
        create(trigger, combo(Some("one"))),
        place(trigger, None),
    ])
    .unwrap();
    let mut hidden = option("one", "box").semantics.unwrap();
    hidden.hidden = true;
    tree.apply(&[Mutation::Patch {
        id: item,
        patch: Patch::Semantics(Change::Set(Some(hidden))),
    }])
    .unwrap();
    assert!(!tree.active(item));
    assert_eq!(tree.resolve_active_descendant(trigger).unwrap(), None);
    assert_eq!(
        tree.validate_event(
            item,
            &Event::Pointer {
                from: item,
                related: None,
                phase: PointerPhase::Move
            }
        ),
        Ok(false)
    );
    assert!(tree.push_event(item, Event::Activate).is_err());
    let stats = tree.stats();
    assert!(tree.apply(&[place(item, None)]).is_err());
    assert_eq!(tree.stats(), stats);
    let mut plain = Descriptor::new(Props::Button(ButtonProps::default()));
    plain.semantics = Some(Box::new(Semantics {
        hidden: true,
        ..Default::default()
    }));
    assert!(plain.validate().is_err());
}

#[test]
fn plan_external_owner_is_checked_before_attachment_and_nested_boundaries_move_as_units() {
    let plan = Plan::new(vec![
        PlanNode {
            parent: None,
            descriptor: Descriptor::new(Props::Group),
        },
        PlanNode {
            parent: Some(0),
            descriptor: option("one", "box"),
        },
    ])
    .unwrap();
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let a = ids.allocate();
    let root = ids.allocate();
    let item = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        create(a, listbox("box")),
        place(a, None),
        Mutation::CreatePlan {
            plan,
            ids: vec![root, item],
        },
    ])
    .unwrap();
    assert!(tree.apply(&[place(root, None)]).is_err());
    tree.apply(&[place(root, Some(a))]).unwrap();
    let outer = ids.allocate();
    let inner = ids.allocate();
    let option_id = ids.allocate();
    let target = ids.allocate();
    tree.apply(&[
        create(outer, Descriptor::new(Props::Group)),
        place(outer, None),
        create(inner, listbox("inner")),
        place(inner, Some(outer)),
        create(option_id, option("nested", "inner")),
        place(option_id, Some(inner)),
        create(target, Descriptor::new(Props::View)),
        place(target, None),
        place(outer, Some(target)),
    ])
    .unwrap();
    assert_eq!(tree.node(option_id).unwrap().parent, Some(inner));
}

#[test]
fn role_owner_and_known_invalid_active_reference_patches_roll_back() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let a = ids.allocate();
    let item = ids.allocate();
    let trigger = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        create(a, listbox("box")),
        place(a, None),
        create(item, option("one", "box")),
        place(item, Some(a)),
        create(trigger, combo(Some("one"))),
        place(trigger, None),
    ])
    .unwrap();
    let stats = tree.stats();
    assert!(tree.prepare_patch(a, &Patch::Role(Change::Clear)).is_err());
    assert_eq!(tree.stats(), stats);
    let mut renamed = listbox("renamed").semantics.unwrap();
    renamed.navigation = Some(Orientation::Vertical);
    assert!(
        tree.prepare_patch(a, &Patch::Semantics(Change::Set(Some(renamed))))
            .is_err()
    );
    assert_eq!(tree.stats(), stats);
    let mut bad = combo(Some("box")).semantics.unwrap();
    bad.expanded = Some(true);
    assert!(
        tree.prepare_patch(trigger, &Patch::Semantics(Change::Set(Some(bad))))
            .is_err()
    );
    assert_eq!(tree.resolve_active_descendant(trigger).unwrap(), Some(item));
    let mut label = Descriptor::new(Props::View);
    label.semantics = Some(Box::new(Semantics {
        id: Some(name("new-box")),
        navigation: Some(Orientation::Vertical),
        ..Default::default()
    }));
    let generic = ids.allocate();
    let button = ids.allocate();
    tree.apply(&[
        create(generic, label),
        place(generic, None),
        create(
            button,
            Descriptor::new(Props::Button(ButtonProps::default())),
        ),
        place(button, Some(generic)),
    ])
    .unwrap();
    assert!(
        tree.prepare_patch(generic, &Patch::Role(Change::Set(Some(Role::Listbox))))
            .is_err()
    );
}

#[test]
fn unplaced_options_and_pending_plan_fields_can_be_configured_before_owner_attachment() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let item = ids.allocate();
    let owner = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[create(item, option("one", "box"))]).unwrap();
    let mut selected = option("one", "box").semantics.unwrap();
    selected.selected = Some(true);
    tree.apply(&[Mutation::Patch {
        id: item,
        patch: Patch::Semantics(Change::Set(Some(selected))),
    }])
    .unwrap();
    assert!(tree.apply(&[place(item, None)]).is_err());
    tree.apply(&[
        create(owner, listbox("box")),
        place(owner, None),
        place(item, Some(owner)),
    ])
    .unwrap();
    let plan = Plan::new(vec![
        PlanNode {
            parent: None,
            descriptor: Descriptor::new(Props::Group),
        },
        PlanNode {
            parent: Some(0),
            descriptor: option("future", "box"),
        },
    ])
    .unwrap();
    let root = ids.allocate();
    let child = ids.allocate();
    tree.apply(&[Mutation::CreatePlan {
        plan,
        ids: vec![root, child],
    }])
    .unwrap();
    let mut meta = option("future", "box").semantics.unwrap();
    meta.selected = Some(true);
    tree.apply(&[
        Mutation::Patch {
            id: child,
            patch: Patch::Semantics(Change::Set(Some(meta))),
        },
        place(root, Some(owner)),
    ])
    .unwrap();
}

#[test]
fn restricted_boundary_scans_are_bounded_while_same_owner_group_reorders_remain_ancestor_only() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let owner = ids.allocate();
    let group = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        create(owner, listbox("box")),
        place(owner, None),
        create(group, Descriptor::new(Props::Group)),
        place(group, Some(owner)),
    ])
    .unwrap();
    for _ in 0..4100 {
        let child = ids.allocate();
        tree.apply(&[
            create(child, Descriptor::new(Props::Text(TextProps::default()))),
            place(child, Some(group)),
        ])
        .unwrap();
    }
    tree.apply(&[place(group, Some(owner))]).unwrap();
    let before = tree.stats();
    assert!(matches!(
        tree.apply(&[place(group, None)]),
        Err(Error::InvalidPlacement(
            "restricted listbox subtree exceeds 4096 live nodes"
        ))
    ));
    assert_eq!(tree.stats(), before);
}
