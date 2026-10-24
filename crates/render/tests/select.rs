use glory_render::*;
use std::rc::Rc;

fn key(value: &str) -> SemanticId {
    SemanticId::new(value).unwrap()
}
fn option(value: &str) -> SelectOption {
    SelectOption {
        key: key(value),
        label: value.into(),
        value: "same-business-value".into(),
        disabled: false,
    }
}
fn select() -> SelectProps {
    SelectProps {
        entries: Rc::new(vec![
            SelectEntry::Option(option("a")),
            SelectEntry::Group(SelectGroup {
                label: "Group".into(),
                disabled: true,
                options: vec![option("b"), option("c")],
            }),
        ]),
        ..Default::default()
    }
}

#[test]
fn finite_configuration_and_keys_are_independent_from_business_values() {
    let mut config = select();
    config.validate().unwrap();
    config.multiple = true;
    config.selection.keys = vec![key("a"), key("b")];
    config.validate().unwrap();
    for keys in [
        vec![key("b"), key("a")],
        vec![key("a"), key("a")],
        vec![key("missing")],
    ] {
        assert!(config.validate_selection(&keys).is_err());
    }
    config.entries = Rc::new(vec![
        SelectEntry::Option(option("a"));
        MAX_SELECT_OPTIONS + 1
    ]);
    assert!(config.validate().is_err());
    let mut config = select();
    config.rows = Some(65);
    assert!(config.validate().is_err());
}

#[test]
fn disabled_options_can_be_program_selected_or_deselected_but_not_newly_user_selected() {
    let mut config = select();
    assert!(config.validate_user_selection(&[key("b")]).is_err());
    config.selection.keys = vec![key("b")];
    config.validate().unwrap();
    config.validate_user_selection(&[key("b")]).unwrap();
    config.validate_user_selection(&[key("a")]).unwrap();
    config.validate_user_selection(&[]).unwrap();
}

#[test]
fn selection_guard_rolls_back_and_keeps_immutable_declaration_shared() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let config = select();
    let entries = config.entries.clone();
    let mut memory = memory::Memory::new(runtime);
    memory
        .apply(&[Mutation::Create {
            id,
            node: Box::new(Descriptor::new(Props::Select(config))),
        }])
        .unwrap();
    let before = memory.stats();
    {
        let _guard = memory
            .prepare_patch(
                id,
                &Patch::SelectSelection {
                    keys: Change::Set(vec![key("a")]),
                    revision: 1,
                },
            )
            .unwrap();
    }
    assert_eq!(before, memory.stats());
    let Props::Select(current) = memory.node(id).unwrap().descriptor.view().props else {
        panic!()
    };
    assert!(current.selection.keys.is_empty());
    assert!(Rc::ptr_eq(&entries, &current.entries));
    memory
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::SelectSelection {
                keys: Change::Set(vec![key("a")]),
                revision: 1,
            },
        }])
        .unwrap();
    assert!(
        memory
            .prepare_patch(
                id,
                &Patch::SelectSelection {
                    keys: Change::Clear,
                    revision: 1
                }
            )
            .is_err()
    );
    let Props::Select(current) = memory.node(id).unwrap().descriptor.view().props else {
        panic!()
    };
    assert_eq!(current.selection.keys, vec![key("a")]);
    assert!(Rc::ptr_eq(&entries, &current.entries));
    memory
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::SelectSelection {
                keys: Change::Keep,
                revision: 999,
            },
        }])
        .unwrap();
    let Props::Select(current) = memory.node(id).unwrap().descriptor.view().props else {
        panic!()
    };
    assert_eq!(current.selection.revision, 1);
}

#[test]
fn select_events_and_field_metadata_have_finite_target_and_revision_rules() {
    let mut descriptor = Descriptor::new(Props::Select(select()));
    descriptor.semantics = Some(Box::new(Semantics {
        required: true,
        invalid: true,
        form_control: Some(FormControl {
            name: "business-name".into(),
            value: None,
        }),
        ..Default::default()
    }));
    descriptor.validate().unwrap();
    EventKind::SelectChanged
        .validate_target(&descriptor)
        .unwrap();
    assert!(
        EventKind::ValueChanged
            .validate_target(&descriptor)
            .is_err()
    );
    assert!(
        Event::SelectChanged {
            keys: vec![key("a")],
            revision: 1
        }
        .validate_payload(NodeIds::new(RuntimeId::new()).allocate(), &descriptor)
        .is_err()
    );
    descriptor.semantics.as_mut().unwrap().readonly = true;
    assert!(matches!(descriptor.validate(), Err(Error::Unsupported(_))));
    descriptor.semantics.as_mut().unwrap().readonly = false;
    descriptor
        .semantics
        .as_mut()
        .unwrap()
        .form_control
        .as_mut()
        .unwrap()
        .value = Some("forbidden".into());
    assert!(descriptor.validate().is_err());
}
