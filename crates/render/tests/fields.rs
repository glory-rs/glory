use glory_render::memory::Memory;
use glory_render::*;
#[test]
fn field_state_targets_validate_and_failed_patches_roll_back() {
    for props in [
        Props::View,
        Props::Button(ButtonProps::default()),
        Props::Text(TextProps::default()),
    ] {
        for state in [
            Semantics {
                required: true,
                ..Default::default()
            },
            Semantics {
                readonly: true,
                ..Default::default()
            },
            Semantics {
                invalid: true,
                ..Default::default()
            },
        ] {
            let mut d = Descriptor::new(props.clone());
            d.semantics = Some(Box::new(state));
            assert!(d.validate().is_err());
        }
    }
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[Mutation::Create {
        id,
        node: Box::new(Descriptor::new(Props::View)),
    }])
    .unwrap();
    let stats = tree.stats();
    assert!(
        tree.prepare_patch(
            id,
            &Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                required: true,
                ..Default::default()
            }))))
        )
        .is_err()
    );
    assert_eq!(tree.stats(), stats);
    assert!(tree.node(id).unwrap().descriptor.view().semantics.is_none());
}
#[test]
fn readonly_validates_revisions_before_discarding_edits_and_allows_program_writes() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut tree = Memory::new(runtime);
    let mut d = Descriptor::new(Props::Input(InputProps {
        value: "draft".into(),
        revision: 2,
        ..Default::default()
    }));
    d.semantics = Some(Box::new(Semantics {
        readonly: true,
        ..Default::default()
    }));
    tree.apply(&[
        Mutation::Create {
            id,
            node: Box::new(d),
        },
        Mutation::Place {
            id,
            parent: None,
            before: None,
        },
    ])
    .unwrap();
    assert!(
        !tree
            .validate_event(
                id,
                &Event::ValueChanged {
                    value: "edit".into(),
                    composing: false,
                    revision: 2
                }
            )
            .unwrap()
    );
    assert!(
        tree.validate_event(
            id,
            &Event::Commit {
                value: "future".into(),
                revision: 3
            }
        )
        .is_err()
    );
    tree.prepare_patch(
        id,
        &Patch::Value {
            value: Change::Set("program".into()),
            revision: 3,
        },
    )
    .unwrap()
    .commit();
    assert!(
        matches!(&tree.node(id).unwrap().descriptor.view().props,Props::Input(p) if p.value=="program")
    );
}
#[test]
fn form_label_and_legend_have_finite_content_and_placement_rules() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let label = ids.allocate();
    let input = ids.allocate();
    let mut tree = Memory::new(runtime);
    let mut d = Descriptor::new(Props::View);
    d.semantics = Some(Box::new(Semantics {
        field_part: Some(FieldPart::Label),
        ..Default::default()
    }));
    tree.apply(&[
        Mutation::Create {
            id: label,
            node: Box::new(d),
        },
        Mutation::Place {
            id: label,
            parent: None,
            before: None,
        },
        Mutation::Create {
            id: input,
            node: Box::new(Descriptor::new(Props::Input(InputProps::default()))),
        },
    ])
    .unwrap();
    assert!(
        tree.apply(&[Mutation::Place {
            id: input,
            parent: Some(label),
            before: None
        }])
        .is_err()
    );
    let legend = ids.allocate();
    let mut d = Descriptor::new(Props::View);
    d.semantics = Some(Box::new(Semantics {
        field_part: Some(FieldPart::Legend),
        ..Default::default()
    }));
    tree.apply(&[Mutation::Create {
        id: legend,
        node: Box::new(d),
    }])
    .unwrap();
    assert!(
        tree.apply(&[Mutation::Place {
            id: legend,
            parent: Some(label),
            before: None
        }])
        .is_err()
    );
}

#[test]
fn structural_form_patches_recheck_existing_children_and_roll_back() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let parent = ids.allocate();
    let child = ids.allocate();
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
            id: child,
            node: Box::new(Descriptor::new(Props::Input(InputProps::default()))),
        },
        Mutation::Place {
            id: child,
            parent: Some(parent),
            before: None,
        },
    ])
    .unwrap();
    let before = tree.receipt();
    let stats = tree.stats();
    assert!(
        tree.prepare_patch(
            parent,
            &Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                field_part: Some(FieldPart::Label),
                ..Default::default()
            }))))
        )
        .is_err()
    );
    assert_eq!(tree.receipt(), before);
    assert_eq!(tree.stats(), stats);
    assert!(
        tree.node(parent)
            .unwrap()
            .descriptor
            .view()
            .semantics
            .is_none()
    );
}
