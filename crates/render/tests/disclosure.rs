use glory_render::{memory::Memory, *};
fn create(tree: &mut Memory, id: NodeId, props: Props) {
    tree.apply(&[Mutation::Create {
        id,
        node: Box::new(Descriptor::new(props)),
    }])
    .unwrap();
}
fn disclosure(target: NodeId, expanded: bool) -> Patch {
    Patch::Disclosure(Change::Set(Some(Disclosure {
        controls: target,
        expanded,
    })))
}
#[test]
fn disclosure_validates_generational_targets_and_rolls_back_without_reindexing() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let target = ids.allocate();
    let button = ids.allocate();
    let invalid = ids.allocate();
    create(&mut tree, target, Props::View);
    create(&mut tree, button, Props::Button(Default::default()));
    create(&mut tree, invalid, Props::Group);
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    for target in [foreign, invalid, button] {
        assert!(
            tree.prepare_patch(button, &disclosure(target, true))
                .is_err()
        );
    }
    assert!(tree.node(button).unwrap().descriptor.disclosure().is_none());
    let stats = tree.stats();
    let receipt = tree.apply(&[]).unwrap();
    {
        let staged = tree
            .prepare_patch(button, &disclosure(target, true))
            .unwrap();
        assert!(staged.descriptor().disclosure().unwrap().expanded);
    }
    assert_eq!(tree.stats(), stats);
    assert_eq!(tree.apply(&[]).unwrap(), receipt);
    assert_eq!(tree.disclosure_referrers(target).count(), 0);
    tree.prepare_patch(button, &disclosure(target, false))
        .unwrap()
        .commit();
    assert_eq!(
        tree.disclosure_referrers(target).collect::<Vec<_>>(),
        vec![button]
    );
    tree.prepare_patch(button, &Patch::Disclosure(Change::Keep))
        .unwrap()
        .commit();
    assert!(
        !tree
            .node(button)
            .unwrap()
            .descriptor
            .disclosure()
            .unwrap()
            .expanded
    );
    tree.prepare_patch(button, &Patch::Disclosure(Change::Clear))
        .unwrap()
        .commit();
    assert_eq!(tree.disclosure_referrers(target).count(), 0);
    tree.prepare_patch(button, &disclosure(target, true))
        .unwrap()
        .commit();
    tree.apply(&[Mutation::Dispose { id: target }]).unwrap();
    assert!(tree.node(button).unwrap().descriptor.disclosure().is_none());
    assert_eq!(tree.disclosure_referrers(target).count(), 0);
    ids.release(target);
    let replacement = ids.allocate();
    assert_eq!(target.slot(), replacement.slot());
    assert_ne!(target, replacement);
    create(&mut tree, replacement, Props::View);
    assert!(
        tree.prepare_patch(button, &disclosure(target, true))
            .is_err()
    );
    assert!(tree.node(button).unwrap().descriptor.disclosure().is_none());
}
#[test]
fn disclosure_create_preflight_and_dispose_owner_order_are_safe() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let target = ids.allocate();
    let button = ids.allocate();
    let props = Props::Button(ButtonProps {
        disclosure: Some(Disclosure {
            expanded: false,
            controls: target,
        }),
        ..Default::default()
    });
    assert!(
        tree.apply(&[Mutation::Create {
            id: button,
            node: Box::new(Descriptor::new(props.clone()))
        }])
        .is_err()
    );
    assert_eq!(tree.len(), 0);
    create(&mut tree, target, Props::View);
    create(&mut tree, button, props);
    tree.apply(&[
        Mutation::Dispose { id: button },
        Mutation::Dispose { id: target },
    ])
    .unwrap();
    assert!(tree.is_empty());
    assert_eq!(tree.disclosure_referrers(target).count(), 0);
}
#[test]
fn unwinding_a_retarget_keeps_index_and_owned_subtree_retirement_clears_referrers() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let parent = ids.allocate();
    let a = ids.allocate();
    let b = ids.allocate();
    let button = ids.allocate();
    for id in [parent, a, b] {
        create(&mut tree, id, Props::View)
    }
    create(&mut tree, button, Props::Button(Default::default()));
    tree.apply(&[
        Mutation::Place {
            id: parent,
            parent: None,
            before: None,
        },
        Mutation::Place {
            id: a,
            parent: Some(parent),
            before: None,
        },
    ])
    .unwrap();
    tree.prepare_patch(button, &disclosure(a, false))
        .unwrap()
        .commit();
    let stats = tree.stats();
    let receipt = tree.apply(&[]).unwrap();
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = tree.prepare_patch(button, &disclosure(b, true)).unwrap();
        panic!("test rollback on unwind");
    }));
    assert!(failed.is_err());
    assert_eq!(tree.stats(), stats);
    assert_eq!(tree.apply(&[]).unwrap(), receipt);
    assert_eq!(
        tree.node(button)
            .unwrap()
            .descriptor
            .disclosure()
            .unwrap()
            .controls,
        a
    );
    assert_eq!(tree.disclosure_referrers(a).count(), 1);
    assert_eq!(tree.disclosure_referrers(b).count(), 0);
    tree.apply(&[Mutation::Dispose { id: parent }]).unwrap();
    assert!(tree.node(button).unwrap().descriptor.disclosure().is_none());
    assert_eq!(tree.disclosure_referrers(a).count(), 0);
}
