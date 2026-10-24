use glory_render::memory::Memory;
use glory_render::*;

fn tip() -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.role = Some(Role::Tooltip);
    d.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("tip").unwrap()),
        layer: Some(Layer {
            modal: false,
            focus: LayerFocus::Preserve,
            placement: LayerPlacement::Anchor(glory_render::AnchorPlacement::new(
                glory_render::AnchorSide::Bottom,
                glory_render::AnchorAlign::Start,
            )),
            anchor: Some(SemanticId::new("target").unwrap()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    d
}
fn create(tree: &mut Memory, id: NodeId, descriptor: Descriptor, parent: Option<NodeId>) {
    tree.apply(&[
        Mutation::Create {
            id,
            node: Box::new(descriptor),
        },
        Mutation::Place {
            id,
            parent,
            before: None,
        },
    ])
    .unwrap();
}
#[test]
fn tooltip_focus_policy_content_and_live_patch_rejections_are_transactional() {
    let descriptor = tip();
    descriptor.validate().unwrap();
    assert_eq!(
        Action::Focus(NodeIds::new(RuntimeId::new()).allocate()).validate_target(&descriptor),
        Err(Error::Unsupported("focus on passive tooltip"))
    );
    for invalid in [
        Layer {
            focus: LayerFocus::Manage,
            modal: false,
            ..Default::default()
        },
        Layer {
            modal: true,
            focus: LayerFocus::Preserve,
            ..Default::default()
        },
        Layer {
            modal: false,
            focus: LayerFocus::Preserve,
            return_focus: Some(SemanticId::new("return").unwrap()),
            ..Default::default()
        },
    ] {
        let mut d = descriptor.clone();
        d.semantics.as_mut().unwrap().layer = Some(invalid);
        assert!(d.validate().is_err());
    }
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let panel = ids.allocate();
    create(&mut tree, panel, descriptor, None);
    let child = ids.allocate();
    create(&mut tree, child, Descriptor::new(Props::View), Some(panel));
    let baseline = tree.receipt();
    for patch in [
        Patch::Focus(Change::Set(FocusMode::Programmatic)),
        Patch::Role(Change::Set(Some(Role::Dialog))),
        Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            navigation: Some(Orientation::Horizontal),
            ..Default::default()
        })))),
    ] {
        assert!(tree.prepare_patch(child, &patch).is_err());
        assert_eq!(tree.receipt(), baseline);
        assert_eq!(
            tree.node(child).unwrap().descriptor.view().focus,
            FocusMode::Auto
        );
        assert_eq!(tree.node(child).unwrap().descriptor.view().role, None);
        assert!(
            tree.node(child)
                .unwrap()
                .descriptor
                .view()
                .semantics
                .is_none()
        );
    }
    assert!(
        Plan::new(vec![
            PlanNode {
                parent: None,
                descriptor: tip()
            },
            PlanNode {
                parent: Some(0),
                descriptor: Descriptor::new(Props::Group)
            },
            PlanNode {
                parent: Some(1),
                descriptor: Descriptor::new(Props::Button(ButtonProps::default()))
            }
        ])
        .is_err()
    );
    let other = ids.allocate();
    create(&mut tree, other, Descriptor::new(Props::View), None);
    let group = ids.allocate();
    create(&mut tree, group, Descriptor::new(Props::Group), Some(other));
    let button = ids.allocate();
    create(
        &mut tree,
        button,
        Descriptor::new(Props::Button(ButtonProps::default())),
        Some(group),
    );
    assert!(
        tree.apply(&[Mutation::Patch {
            id: other,
            patch: Patch::Role(Change::Set(Some(Role::Tooltip)))
        }])
        .is_err()
    );
    assert_eq!(tree.node(other).unwrap().descriptor.view().role, None);
    assert!(
        tree.apply(&[Mutation::Place {
            id: group,
            parent: Some(panel),
            before: None
        }])
        .is_err()
    );
    tree.apply(&[Mutation::Suspend { id: group }]).unwrap();
    assert!(
        tree.apply(&[Mutation::Resume {
            id: group,
            parent: Some(panel),
            before: None
        }])
        .is_err()
    );
    assert_eq!(tree.node(group).unwrap().parent, Some(other));
}

#[test]
fn pointer_within_direct_validation_checks_physical_origins_common_ancestry_and_retirement() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let root = ids.allocate();
    create(&mut tree, root, Descriptor::new(Props::Group), None);
    let first = ids.allocate();
    create(&mut tree, first, Descriptor::new(Props::View), Some(root));
    let second = ids.allocate();
    create(&mut tree, second, Descriptor::new(Props::View), Some(root));
    let outside = ids.allocate();
    create(&mut tree, outside, Descriptor::new(Props::View), None);
    let event = |from, related| Event::PointerWithin {
        from,
        related,
        phase: PointerPhase::Enter,
    };
    assert!(
        tree.validate_event(root, &event(first, Some(outside)))
            .unwrap()
    );
    assert!(
        !tree
            .validate_event(root, &event(first, Some(second)))
            .unwrap()
    );
    assert!(tree.validate_event(root, &event(outside, None)).is_err());
    assert!(tree.validate_event(root, &event(root, None)).is_err());
    assert!(
        tree.validate_event(root, &event(first, Some(root)))
            .is_err()
    );
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    assert_eq!(
        tree.validate_event(root, &event(first, Some(foreign))),
        Err(Error::ForeignRuntime)
    );
    tree.apply(&[Mutation::Suspend { id: first }]).unwrap();
    assert!(!tree.validate_event(root, &event(first, None)).unwrap());
    tree.apply(&[Mutation::Dispose { id: outside }]).unwrap();
    assert!(
        !tree
            .validate_event(root, &event(second, Some(outside)))
            .unwrap()
    );
}
