use glory_render::{
    memory::{Memory, Subtree},
    *,
};
fn separator(axis: Orientation) -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.role = Some(Role::Separator);
    d.semantics = Some(Box::new(Semantics {
        separator_axis: Some(axis),
        ..Default::default()
    }));
    d
}
#[test]
fn separator_orientation_focus_and_interests_have_finite_targets() {
    let mut d = separator(Orientation::Horizontal);
    d.validate().unwrap();
    let original = d.clone();
    assert!(
        d.patch(Patch::Focus(Change::Set(FocusMode::TabStop)))
            .is_err()
    );
    assert_eq!(d, original);
    assert!(
        d.patch(Patch::Role(Change::Set(Some(Role::Group))))
            .is_err()
    );
    assert_eq!(d, original);
    let mut plain = Descriptor::new(Props::View);
    assert!(
        plain
            .patch(Patch::Semantics(Change::Set(original.semantics.clone())))
            .is_err()
    );
    let id = NodeIds::new(RuntimeId::new()).allocate();
    assert!(Action::Focus(id).validate_target(&d).is_err());
    for kind in [
        EventKind::Activate,
        EventKind::Keyboard,
        EventKind::Drag,
        EventKind::Navigate,
    ] {
        assert!(kind.validate_target(&d).is_err());
    }
    EventKind::Resize.validate_target(&d).unwrap();
}
#[test]
fn separators_reject_children_and_container_to_separator_patches_without_side_effects() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let parent = ids.allocate();
    let child = ids.allocate();
    let separator_id = ids.allocate();
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
            node: Box::new(Descriptor::new(Props::View)),
        },
        Mutation::Place {
            id: child,
            parent: Some(parent),
            before: None,
        },
        Mutation::Create {
            id: separator_id,
            node: Box::new(separator(Orientation::Vertical)),
        },
        Mutation::Place {
            id: separator_id,
            parent: None,
            before: None,
        },
    ])
    .unwrap();
    let stats = tree.stats();
    let receipt = tree.receipt();
    assert!(
        tree.prepare_patch(parent, &Patch::Role(Change::Set(Some(Role::Separator))))
            .is_err()
    );
    assert_eq!(tree.node(parent).unwrap().descriptor.view().role, None);
    assert!(
        tree.validate_child(Subtree::Existing(child), Some(separator_id))
            .is_err()
    );
    assert!(
        tree.apply(&[Mutation::Place {
            id: child,
            parent: Some(separator_id),
            before: None
        }])
        .is_err()
    );
    assert_eq!(tree.stats(), stats);
    assert_eq!(tree.receipt(), receipt);
    assert!(
        Plan::new(vec![
            PlanNode {
                parent: None,
                descriptor: separator(Orientation::Horizontal)
            },
            PlanNode {
                parent: Some(0),
                descriptor: Descriptor::new(Props::Group)
            }
        ])
        .is_err()
    );
}
